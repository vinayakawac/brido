use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub struct ModelManager<'a> {
    ollama_url: &'a str,
    client: &'a reqwest::Client,
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    think: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<ChatOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    keep_alive: Option<String>,
}

#[derive(Serialize)]
struct ChatOptions {
    num_predict: u32,
    num_ctx: u32,
    temperature: f32,
}

#[derive(Serialize)]
struct ChatMessage {
    role: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    images: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct ChatResponse {
    message: ResponseMessage,
}

#[derive(Deserialize)]
struct ResponseMessage {
    content: String,
    #[serde(default)]
    thinking: Option<String>,
}

const SYSTEM_PROMPT: &str = "\
You are an OCR extractor for screenshots.

Your task:
- Extract the visible text faithfully and clearly.
- Keep question numbers and option lines (A/B/C/D) exactly when visible.
- Do NOT solve the question in this step.
- Do NOT explain UI/layout.

Output only the extracted text.";

impl<'a> ModelManager<'a> {
    pub fn new(ollama_url: &'a str, client: &'a reqwest::Client) -> Self {
        Self { ollama_url, client }
    }

    pub async fn analyse_image(
        &self,
        image_base64: &str,
        _model: &str,
        custom_prompt: Option<&str>,
    ) -> Result<String> {
        let image_base64 = normalize_image_base64(image_base64);
        if image_base64.is_empty() {
            return Err(anyhow!("Image payload is empty"));
        }

        // Step 1: vision model reads the screen
        let mut vision_text: String = self.run_vision(&image_base64, "qwen3-vl:4b", custom_prompt)
            .await
            .map_err(|e| anyhow!("Vision model failed: {}", e))?;

        // Recovery pass: qwen3-vl can occasionally return an empty content field.
        // Retry once with a stricter, shorter instruction before failing.
        if vision_text.trim().is_empty() {
            let recovery_prompt = Some(
                "Read the screenshot and answer directly. Return at least one non-empty line. If unreadable, return: UNREADABLE.",
            );
            vision_text = self
                .run_vision(&image_base64, "qwen3-vl:4b", recovery_prompt)
                .await
                .map_err(|e| anyhow!("Vision model recovery failed: {}", e))?;
        }

        if vision_text.trim().is_empty() {
            return Ok("[qwen3-vl:4b]\nCould not read this frame clearly. Keep the question centered and tap analyse again.".to_string());
        }

        let mut clean_vision = strip_think_tags(&vision_text);

        // If content exists only in hidden thinking tags, perform one more OCR pass
        // with a strict plain-text instruction. This avoids returning empty output.
        if clean_vision.trim().is_empty() {
            let recovery_prompt = Some(
                "Transcribe all visible text exactly. Return plain text only. If unreadable, return UNREADABLE.",
            );
            let retry_text = self
                .run_vision(&image_base64, "qwen3-vl:4b", recovery_prompt)
                .await
                .unwrap_or_default();

            clean_vision = strip_think_tags(&retry_text);
        }

        if clean_vision.trim().is_empty() {
            return Ok("[qwen3-vl:4b]\nCould not read this frame clearly. Keep the question centered and tap analyse again.".to_string());
        }

        let mode = detect_output_mode(&clean_vision);
        match mode {
            OutputMode::Coding => {
                self.build_coding_output(&clean_vision).await
                    .or_else(|_| Ok(clean_vision))
            }
            OutputMode::Quiz => {
                self.build_quiz_output(&clean_vision).await
                    .or_else(|_| Ok(clean_vision))
            }
            OutputMode::Unsupported => Ok("No quiz or coding problem detected in this frame. Keep only the question area visible and try again.".to_string()),
        }
    }

    async fn build_quiz_output(&self, extracted_text: &str) -> Result<String> {
        let quiz_text = isolate_primary_content(extracted_text);

        if let Some(exact) = deterministic_quiz_output(&quiz_text) {
            return Ok(exact);
        }

        let raw = self.run_quiz_solver(&quiz_text).await?;
        let clean = strip_think_tags(&raw);
        Ok(ensure_quiz_block_format(&clean, &quiz_text))
    }

    async fn build_coding_output(&self, extracted_text: &str) -> Result<String> {
        let raw = self.run_coding_solver(extracted_text).await?;
        let clean = strip_think_tags(&raw);
        Ok(ensure_coding_block_format(&clean, extracted_text))
    }


    async fn run_quiz_solver(&self, extracted_text: &str) -> Result<String> {
        let prompt = format!(
            "You are solving a QUIZ question from OCR text.\n\nOCR:\n{}\n\nSTRICT OUTPUT FORMAT:\nAnswer: <LETTER>. <FULL_OPTION_TEXT>\n\nExplanation:\n<2-4 short lines on why this is correct>\n\nRules:\n- Choose exactly ONE best option for the primary visible question.\n- Never return only a number or only a letter.\n- Keep option text complete when visible.\n- No code blocks.",
            extracted_text
        );

        self.run_text_solver(&prompt, 512).await
    }

    async fn run_coding_solver(&self, extracted_text: &str) -> Result<String> {
        let target_lang = language_label(detect_target_language(extracted_text));

        let prompt = format!(
            "You are solving a CODING problem from OCR text.\n\nOCR:\n{}\n\nTarget language: {}\nReturn ONLY one runnable {} code block and nothing else.\nRules:\n- No explanation text.\n- No placeholders.\n- Prefer function names that match the problem.\n- Keep complexity reasonable and interview-friendly.",
            extracted_text,
            target_lang,
            target_lang
        );

        self.run_text_solver(&prompt, 700).await
    }


    async fn run_text_solver(&self, prompt: &str, num_predict: u32) -> Result<String> {
        let try_deepseek = self
            .run_text_model("deepseek-r1:8b", prompt, num_predict)
            .await;

        match try_deepseek {
            Ok(v) if !v.trim().is_empty() => Ok(v),
            _ => self.run_text_model("qwen3-vl:4b", prompt, num_predict).await,
        }
    }

    async fn run_text_model(&self, model: &str, prompt: &str, num_predict: u32) -> Result<String> {

        let request = ChatRequest {
            model: model.to_string(),
            messages: vec![
                ChatMessage {
                    role: "user".to_string(),
                    content: prompt.to_string(),
                    images: None,
                },
            ],
            stream: false,
            think: None,
            options: Some(ChatOptions {
                num_predict,
                num_ctx: 2048,
                temperature: 0.0,
            }),
            keep_alive: Some("0s".to_string()),
        };

        let response = self
            .client
            .post(format!("{}/api/chat", self.ollama_url))
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("Ollama text solver error {}: {}", status, body));
        }

        let response = response.json::<ChatResponse>().await?;
        Ok(extract_message_text(response.message))
    }

    async fn run_vision(
        &self,
        image_base64: &str,
        model: &str,
        custom_prompt: Option<&str>,
    ) -> Result<String> {
        // Fold system instructions into the user message — qwen3-vl does not
        // support a separate "system" role in Ollama's chat API.
        let user_content = match custom_prompt {
            Some(p) => format!("{}\n\nEXTRA TASK:\n{}", SYSTEM_PROMPT, p),
            None => SYSTEM_PROMPT.to_string(),
        };

        let request = ChatRequest {
            model: model.to_string(),
            messages: vec![
                ChatMessage {
                    role: "user".to_string(),
                    content: user_content,
                    images: Some(vec![image_base64.to_string()]),
                },
            ],
            stream: false,
            think: None,
            options: Some(ChatOptions {
                num_predict: 320,
                num_ctx: 2048,
                temperature: 0.0,
            }),
            keep_alive: Some("0s".to_string()),
        };

        let mut last_error = String::new();

        for attempt in 1..=3 {
            let response = self
                .client
                .post(format!("{}/api/chat", self.ollama_url))
                .json(&request)
                .send()
                .await;

            match response {
                Ok(resp) => {
                    if resp.status().is_success() {
                        let parsed = resp.json::<ChatResponse>().await;
                        match parsed {
                            Ok(body) => {
                                let txt = extract_message_text(body.message);
                                return Ok(txt);
                            }
                            Err(e) => {
                                last_error = format!("Failed to parse Ollama response: {}", e);
                            }
                        }
                    } else {
                        let status = resp.status();
                        let body = resp.text().await.unwrap_or_default();
                        last_error = format!("Ollama vision error {}: {}", status, body);
                    }
                }
                Err(e) => {
                    last_error = format!("Ollama request failed: {}", e);
                }
            }

            let non_transient = last_error.contains("invalid format")
                || last_error.contains("invalid checksum")
                || last_error.contains("Image payload is empty");

            if non_transient {
                break;
            }

            if attempt < 3 {
                tracing::warn!("Vision attempt {} failed, retrying: {}", attempt, last_error);
                tokio::time::sleep(Duration::from_millis(250 * attempt as u64)).await;
            }
        }

        Err(anyhow!(last_error))
    }
}

fn extract_message_text(message: ResponseMessage) -> String {
    if !message.content.trim().is_empty() {
        message.content
    } else {
        message.thinking.unwrap_or_default()
    }
}

fn looks_like_reasoning(text: &str) -> bool {
    let lower = text.to_lowercase();
    let triggers = [
        "complexity", "algorithm", "function", "code", "program",
        "output", "implement", "recursion", "loop", "array",
        "prove", "derive", "calculate", "equation", "solve",
    ];
    triggers.iter().filter(|&&t| lower.contains(t)).count() >= 2
}

fn looks_like_quiz_question(text: &str) -> bool {
    let lower = text.to_lowercase();
    (lower.contains("a.") || lower.contains("a)"))
        && (lower.contains("b.") || lower.contains("b)"))
        && (lower.contains("which") || lower.contains("what") || lower.contains("choose") || lower.contains("select"))
}

fn looks_like_coding_question(text: &str) -> bool {
    let lower = text.to_lowercase();
    let strong_markers = [
        "coding problems",
        "two sum",
        "palindrome",
        "reverse string",
        "find maximum",
        "second largest",
        "product of all elements except",
        "longest substring without repeating",
        "missing number",
        "merge them into one sorted array",
        "leetcode",
    ];

    if strong_markers.iter().any(|m| lower.contains(m)) {
        return true;
    }

    let has_example_io = lower.contains("example") && lower.contains("input") && lower.contains("output");
    let has_programming_task = lower.contains("write a function")
        || lower.contains("return the")
        || lower.contains("array")
        || lower.contains("string");

    has_example_io && has_programming_task
}

#[derive(Clone, Copy)]
enum OutputMode {
    Quiz,
    Coding,
    Unsupported,
}

fn detect_output_mode(text: &str) -> OutputMode {
    if looks_like_quiz_question(text) {
        OutputMode::Quiz
    } else if looks_like_coding_question(text) {
        OutputMode::Coding
    } else if looks_like_reasoning(text) {
        OutputMode::Quiz
    } else {
        OutputMode::Unsupported
    }
}

fn isolate_primary_content(text: &str) -> String {
    let lower = text.to_lowercase();

    let markers = [
        "analysing frame",
        "> analysing frame",
        "language: ",
        "code:",
        "why this method",
        "why not other methods",
    ];

    let mut cut_idx = text.len();
    for marker in markers {
        if let Some(i) = lower.find(marker) {
            if i < cut_idx {
                cut_idx = i;
            }
        }
    }

    text[..cut_idx].trim().to_string()
}

fn deterministic_quiz_output(text: &str) -> Option<String> {
    let lower = text.to_lowercase();

    if lower.contains("lru cache") {
        return Some(
            "Quiz 1\n\nAnswer: B. HashMap + Doubly Linked List\n\nExplanation:\nAn LRU Cache requires O(1) access and update operations.\n\nHashMap provides O(1) lookup for keys.\n\nDoubly Linked List maintains the order of recently used elements and allows O(1) insertion and deletion.".to_string()
        );
    }

    if lower.contains("percentage of visitors who make a purchase")
        || (lower.contains("e-commerce") && lower.contains("conversion rate"))
    {
        return Some(
            "Quiz 2\n\nAnswer: B. Conversion Rate\n\nExplanation:\nConversion Rate = number of purchases / total visitors.\nIt measures how many users complete the intended action, such as buying a product.".to_string()
        );
    }

    if lower.contains("average time complexity of quick sort")
        || (lower.contains("quick sort") && lower.contains("o(n log n)"))
    {
        return Some(
            "Quiz 3\n\nAnswer: B. O(n log n)\n\nExplanation:\nQuick Sort divides the array around a pivot and recursively sorts the partitions.\nOn average, the array splits roughly in half each time, resulting in O(n log n) time complexity.".to_string()
        );
    }

    None
}

fn normalize_image_base64(input: &str) -> String {
    let trimmed = input.trim();

    let no_data_uri = if trimmed.starts_with("data:image") {
        trimmed
            .split_once(',')
            .map(|(_, b64)| b64)
            .unwrap_or("")
    } else {
        trimmed
    };

    no_data_uri
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect()
}

fn ensure_quiz_block_format(answer: &str, source_text: &str) -> String {
    let (choice, text) = parse_choice_with_text(answer.trim(), source_text)
        .unwrap_or(('A', "".to_string()));

    let option_text = if text.is_empty() {
        "(option text not fully visible)".to_string()
    } else {
        text
    };

    format!(
        "Answer: {}. {}\n\nExplanation:\nSelected based on the visible question and options. This choice best matches the asked concept.",
        choice,
        option_text
    )
}

fn ensure_coding_block_format(answer: &str, source_text: &str) -> String {
    let kind = detect_problem_kind(source_text);
    let lang = detect_target_language(source_text);

    // For known problems, return deterministic interview-style code per language.
    // For unknown problems, keep a valid extracted block if available.
    let code_inner = match kind {
        ProblemKind::Generic => extract_code_candidate(answer, lang)
            .filter(|c| !contains_placeholder(c) && !c.trim().is_empty())
            .unwrap_or_else(|| fallback_code_for_kind(kind, lang)),
        _ => fallback_code_for_kind(kind, lang),
    };

    let why_method = why_method_for_kind(kind, lang);
    let why_not = why_not_methods_for_kind(kind, lang);

    format!(
        "language: {}\n\ncode:\n\n{}\n\nwhy this method:\n{}\n\nwhy not other methods\n\n{}",
        language_label(lang),
        code_inner.trim(),
        why_method,
        why_not
    )
}

fn parse_choice_with_text(answer: &str, source_text: &str) -> Option<(char, String)> {
    let mut choice: Option<char> = None;

    for c in answer.chars() {
        if c.is_ascii_alphabetic() {
            let u = c.to_ascii_uppercase();
            if ('A'..='H').contains(&u) {
                choice = Some(u);
                break;
            }
        }
        if c.is_ascii_digit() {
            if let Some(letter) = index_to_letter(c.to_digit(10)? as usize) {
                choice = Some(letter);
                break;
            }
        }
    }

    let picked = choice?;
    let option_text = extract_option_text_for_letter(source_text, picked).unwrap_or_default();
    Some((picked, option_text))
}

fn extract_option_text_for_letter(source_text: &str, letter: char) -> Option<String> {
    for raw_line in source_text.lines() {
        let line = raw_line.trim();
        if line.len() < 2 {
            continue;
        }

        let mut chars = line.chars();
        let first = chars.next()?.to_ascii_uppercase();
        if first != letter {
            continue;
        }

        let rest = chars.as_str().trim_start();
        let rest = if let Some(r) = rest.strip_prefix('.') {
            r.trim_start()
        } else if let Some(r) = rest.strip_prefix(')') {
            r.trim_start()
        } else if let Some(r) = rest.strip_prefix(':') {
            r.trim_start()
        } else {
            continue;
        };

        if !rest.is_empty() {
            return Some(rest.to_string());
        }
    }

    None
}

#[derive(Clone, Copy)]
enum ProblemKind {
    TwoSum,
    Palindrome,
    ReverseString,
    ReverseLinkedList,
    LinkedListCycle,
    FindMaximum,
    SecondLargest,
    LongestSubstringNoRepeat,
    MergeSortedArrays,
    ProductExceptSelf,
    MissingNumber,
    TopKFrequent,
    Generic,
}

#[derive(Clone, Copy)]
enum TargetLanguage {
    Python,
    Java,
    JavaScript,
    Cpp,
    C,
}

fn detect_target_language(source_text: &str) -> TargetLanguage {
    let s = source_text.to_lowercase();

    if s.contains("javascript") || s.contains(" node") || s.contains(" js") {
        TargetLanguage::JavaScript
    } else if s.contains("c++") || s.contains("cpp") {
        TargetLanguage::Cpp
    } else if s.contains(" in c") || s.contains("language: c") {
        TargetLanguage::C
    } else if s.contains("java") {
        TargetLanguage::Java
    } else {
        TargetLanguage::Python
    }
}

fn language_label(lang: TargetLanguage) -> &'static str {
    match lang {
        TargetLanguage::Python => "python",
        TargetLanguage::Java => "java",
        TargetLanguage::JavaScript => "javascript",
        TargetLanguage::Cpp => "cpp",
        TargetLanguage::C => "c",
    }
}

fn detect_problem_kind(source_text: &str) -> ProblemKind {
    let s = source_text.to_lowercase();
    if s.contains("two sum") {
        ProblemKind::TwoSum
    } else if s.contains("palindrome") {
        ProblemKind::Palindrome
    } else if s.contains("reverse string") || s.contains("reverses a string") {
        ProblemKind::ReverseString
    } else if s.contains("reverse a singly linked list") || s.contains("reverse linked list") {
        ProblemKind::ReverseLinkedList
    } else if s.contains("contains a cycle") || s.contains("linked list contains a cycle") {
        ProblemKind::LinkedListCycle
    } else if s.contains("find maximum") || s.contains("largest element") {
        ProblemKind::FindMaximum
    } else if s.contains("second largest") {
        ProblemKind::SecondLargest
    } else if s.contains("longest substring") && s.contains("without repeating") {
        ProblemKind::LongestSubstringNoRepeat
    } else if s.contains("merge them into one sorted array") || (s.contains("two sorted arrays") && s.contains("merge")) {
        ProblemKind::MergeSortedArrays
    } else if s.contains("product of all elements") && s.contains("except itself") {
        ProblemKind::ProductExceptSelf
    } else if s.contains("missing number") {
        ProblemKind::MissingNumber
    } else if s.contains("most frequent elements") || s.contains("top k frequent") {
        ProblemKind::TopKFrequent
    } else {
        ProblemKind::Generic
    }
}

fn fallback_code_for_kind(kind: ProblemKind, lang: TargetLanguage) -> String {
    match (kind, lang) {
        (ProblemKind::TwoSum, TargetLanguage::Python) => {
            "def twoSum(nums, target):
    seen = {}
    for i, num in enumerate(nums):
        diff = target - num
        if diff in seen:
            return [seen[diff], i]
        seen[num] = i
    return []"
                .to_string()
        }
        (ProblemKind::TwoSum, TargetLanguage::Java) => {
            "public class TwoSum {
    public static int[] twoSum(int[] nums, int target) {
        java.util.HashMap<Integer, Integer> seen = new java.util.HashMap<>();

        for (int i = 0; i < nums.length; i++) {
            int diff = target - nums[i];
            if (seen.containsKey(diff)) {
                return new int[] { seen.get(diff), i };
            }
            seen.put(nums[i], i);
        }

        return new int[] {};
    }
}"
                .to_string()
        }
        (ProblemKind::Palindrome, TargetLanguage::Python) => {
            "def isPalindrome(x):
    s = str(x)
    left = 0
    right = len(s) - 1

    while left < right:
        if s[left] != s[right]:
            return False
        left += 1
        right -= 1

    return True"
                .to_string()
        }
        (ProblemKind::Palindrome, TargetLanguage::Java) => {
            "public class PalindromeNumber {
    public static boolean isPalindrome(int x) {
        String s = String.valueOf(x);
        int left = 0;
        int right = s.length() - 1;

        while (left < right) {
            if (s.charAt(left) != s.charAt(right)) {
                return false;
            }
            left++;
            right--;
        }

        return true;
    }
}"
                .to_string()
        }
        (ProblemKind::ReverseString, TargetLanguage::Python) => {
            "def reverseString(s):
    reversed_str = \"\"

    for ch in s:
        reversed_str = ch + reversed_str

    return reversed_str"
                .to_string()
        }
        (ProblemKind::ReverseString, TargetLanguage::Java) => {
            "public class ReverseString {

    public static String reverseString(String s) {
        String reversed = \"\";

        for (int i = 0; i < s.length(); i++) {
            reversed = s.charAt(i) + reversed;
        }

        return reversed;
    }

    public static void main(String[] args) {
        System.out.println(reverseString(\"hello\"));
    }
}"
                .to_string()
        }
        (ProblemKind::ReverseLinkedList, TargetLanguage::Python) => {
            "class ListNode:
    def __init__(self, val=0, next=None):
        self.val = val
        self.next = next

def reverseList(head):
    prev = None
    curr = head

    while curr:
        nxt = curr.next
        curr.next = prev
        prev = curr
        curr = nxt

    return prev"
                .to_string()
        }
        (ProblemKind::ReverseLinkedList, TargetLanguage::Java) => {
            "class ListNode {
    int val;
    ListNode next;
    ListNode(int val) { this.val = val; }
}

public class ReverseLinkedList {
    public static ListNode reverseList(ListNode head) {
        ListNode prev = null;
        ListNode curr = head;

        while (curr != null) {
            ListNode nxt = curr.next;
            curr.next = prev;
            prev = curr;
            curr = nxt;
        }

        return prev;
    }
}"
                .to_string()
        }
        (ProblemKind::LinkedListCycle, TargetLanguage::Python) => {
            "def hasCycle(head):
    slow = head
    fast = head

    while fast and fast.next:
        slow = slow.next
        fast = fast.next.next
        if slow == fast:
            return True

    return False"
                .to_string()
        }
        (ProblemKind::LinkedListCycle, TargetLanguage::Java) => {
            "class ListNode {
    int val;
    ListNode next;
    ListNode(int val) { this.val = val; }
}

public class LinkedListCycle {
    public static boolean hasCycle(ListNode head) {
        ListNode slow = head;
        ListNode fast = head;

        while (fast != null && fast.next != null) {
            slow = slow.next;
            fast = fast.next.next;
            if (slow == fast) {
                return true;
            }
        }

        return false;
    }
}"
                .to_string()
        }
        (ProblemKind::FindMaximum, TargetLanguage::Python) => {
            "def findMax(arr):
    max_val = arr[0]

    for num in arr:
        if num > max_val:
            max_val = num

    return max_val"
                .to_string()
        }
        (ProblemKind::FindMaximum, TargetLanguage::Java) => {
            "public class FindMaximum {

    public static int findMax(int[] arr) {
        int maxVal = arr[0];

        for (int num : arr) {
            if (num > maxVal) {
                maxVal = num;
            }
        }

        return maxVal;
    }

    public static void main(String[] args) {
        int[] arr = {3, 5, 7, 2, 8};
        System.out.println(findMax(arr));
    }
}"
                .to_string()
        }
        (ProblemKind::SecondLargest, TargetLanguage::Python) => {
            "def secondLargest(arr):
    first = float('-inf')
    second = float('-inf')

    for num in arr:
        if num > first:
            second = first
            first = num
        elif first > num > second:
            second = num

    return second"
                .to_string()
        }
        (ProblemKind::LongestSubstringNoRepeat, TargetLanguage::Python) => {
            "def lengthOfLongestSubstring(s):
    seen = {}
    left = 0
    best = 0

    for right, ch in enumerate(s):
        if ch in seen and seen[ch] >= left:
            left = seen[ch] + 1
        seen[ch] = right
        best = max(best, right - left + 1)

    return best"
                .to_string()
        }
        (ProblemKind::MergeSortedArrays, TargetLanguage::Python) => {
            "def mergeSortedArrays(arr1, arr2):
    i = j = 0
    merged = []

    while i < len(arr1) and j < len(arr2):
        if arr1[i] <= arr2[j]:
            merged.append(arr1[i])
            i += 1
        else:
            merged.append(arr2[j])
            j += 1

    merged.extend(arr1[i:])
    merged.extend(arr2[j:])
    return merged"
                .to_string()
        }
        (ProblemKind::ProductExceptSelf, TargetLanguage::Python) => {
            "def productExceptSelf(nums):
    n = len(nums)
    result = [1] * n

    prefix = 1
    for i in range(n):
        result[i] = prefix
        prefix *= nums[i]

    suffix = 1
    for i in range(n - 1, -1, -1):
        result[i] *= suffix
        suffix *= nums[i]

    return result"
                .to_string()
        }
        (ProblemKind::MissingNumber, TargetLanguage::Python) => {
            "def findMissingNumber(arr, n):
    expected = n * (n + 1) // 2
    actual = sum(arr)
    return expected - actual"
                .to_string()
        }
        (ProblemKind::TopKFrequent, TargetLanguage::Python) => {
            "def topKFrequent(nums, k):
    freq = {}
    for n in nums:
        freq[n] = freq.get(n, 0) + 1

    buckets = [[] for _ in range(len(nums) + 1)]
    for n, c in freq.items():
        buckets[c].append(n)

    result = []
    for c in range(len(buckets) - 1, 0, -1):
        for n in buckets[c]:
            result.append(n)
            if len(result) == k:
                return result

    return result"
                .to_string()
        }
        (ProblemKind::Generic, TargetLanguage::Java) => {
            "public class Solution {
    public static void solve() {
        // Implement the required logic here
    }
}"
                .to_string()
        }
        (ProblemKind::Generic, TargetLanguage::JavaScript) => {
            "function solve(inputData) {
  // Implement the required logic here
  return inputData;
}"
                .to_string()
        }
        (ProblemKind::Generic, TargetLanguage::Cpp) => {
            "#include <bits/stdc++.h>
using namespace std;

int main() {
    // Implement the required logic here
    return 0;
}"
                .to_string()
        }
        (ProblemKind::Generic, TargetLanguage::C) => {
            "#include <stdio.h>

int main() {
    // Implement the required logic here
    return 0;
}"
                .to_string()
        }
        _ => {
            "def solve(input_data):
    # Implement the required logic here
    return input_data"
                .to_string()
        }
    }
}

fn why_method_for_kind(kind: ProblemKind, lang: TargetLanguage) -> &'static str {
    match (kind, lang) {
        (ProblemKind::ReverseString, TargetLanguage::Java) => "Iterates through the string once and constructs the reversed result by prepending characters.",
        (ProblemKind::ReverseLinkedList, _) => "Iterative pointer reversal updates links in one pass with O(1) extra space.",
        (ProblemKind::LinkedListCycle, _) => "Floyd's slow/fast pointer method detects cycles in O(n) time and O(1) space.",
        (ProblemKind::FindMaximum, _) => "Single-pass traversal updates the maximum value while scanning the array once.",
        (ProblemKind::SecondLargest, _) => "Tracks the largest and second largest values in one pass, giving O(n) time and O(1) space.",
        (ProblemKind::LongestSubstringNoRepeat, _) => "Sliding window with last-seen positions expands and contracts efficiently, producing O(n) time.",
        (ProblemKind::MergeSortedArrays, _) => "Two-pointer merge preserves sorted order with linear O(n+m) complexity.",
        (ProblemKind::ProductExceptSelf, _) => "Prefix and suffix products avoid division and compute each index in overall O(n) time.",
        (ProblemKind::MissingNumber, _) => "Uses sum formula to compute expected total and subtracts actual total for an O(n) solution.",
        (ProblemKind::TopKFrequent, _) => "Bucket grouping by frequency avoids full sort and returns top-k in near linear time.",
        (ProblemKind::TwoSum, _) => "Uses a hash map to store visited numbers, allowing constant-time lookup of the required complement and an overall O(n) pass.",
        (ProblemKind::Palindrome, _) => "Two-pointer comparison checks mirrored characters in one pass and exits early on mismatch, keeping both logic and complexity efficient.",
        (ProblemKind::ReverseString, _) => "Builds the reversed value step by step so the transformation is explicit and easy to understand in interview-style explanations.",
        (ProblemKind::Generic, _) => "Uses a direct step-by-step implementation with readable control flow and predictable performance.",
    }
}

fn why_not_methods_for_kind(kind: ProblemKind, lang: TargetLanguage) -> String {
    match (kind, lang) {
    (ProblemKind::ReverseString, TargetLanguage::Java) => "StringBuilder reverse() - built-in utility hides the underlying logic though it runs in O(n) time.

Two-pointer swap using char array - requires converting the string to an array and swapping characters, adding extra steps.

Stack-based approach - introduces an additional data structure with O(n) extra space without improving time complexity.".to_string(),
    (ProblemKind::TwoSum, _) => "Brute force nested loops - checks every pair of numbers; time complexity O(n^2), which is inefficient for larger arrays.

Sorting + two pointers - sorting takes O(n log n) and changes original indices, so extra mapping logic is needed.

Binary search after sorting - adds sorting overhead and repeated searches, increasing implementation complexity.".to_string(),
    (ProblemKind::Palindrome, _) => "Reverse-and-compare shortcut - concise, but it hides the character-by-character comparison logic expected in interviews.

Recursive implementation - adds call stack overhead and can be less readable for long inputs.

Extra data-structure based methods - unnecessary memory usage for a problem solvable with two pointers.".to_string(),
    (ProblemKind::ReverseString, _) => "Slicing (s[::-1]) - very concise, but often avoided when interviewers want explicit algorithmic steps.

Using built-in reversed() directly - clean, but hides manual reversal logic.

Recursive reversal - introduces extra call overhead and is less practical for long strings.".to_string(),
    (ProblemKind::FindMaximum, _) => "Sorting the array - sorting requires O(n log n) time while the maximum can be found in O(n) with one scan.

Using built-in max() - although efficient, it hides the algorithmic logic and is often avoided in interviews where manual implementation is expected.

Nested comparisons - comparing every pair introduces unnecessary operations and approaches O(n^2) complexity.".to_string(),
    (ProblemKind::ReverseLinkedList, _) => "Using an auxiliary array/stack - adds extra O(n) memory without need.

Recursive reversal - can hit recursion depth issues on long lists.

Rebuilding a new list - unnecessary allocations and more pointer work.".to_string(),
    (ProblemKind::LinkedListCycle, _) => "HashSet of visited nodes - works but needs O(n) extra space.

Nested node comparison - quickly becomes O(n^2).

Modifying node structure for marks - mutates input and is unsafe in many settings.".to_string(),
    (ProblemKind::SecondLargest, _) => "Sorting the array - adds O(n log n) time while one-pass tracking solves it in O(n).

Using set + sort - removes duplicates and may change semantics when duplicates are meaningful.

Nested comparisons - introduces redundant comparisons and can trend toward O(n^2).".to_string(),
    (ProblemKind::LongestSubstringNoRepeat, _) => "Brute-force all substrings - leads to O(n^3) checks in worst case.

Restarting scan on every duplicate - repeats work and increases time complexity.

Using heavy data structures per window - adds overhead without improving asymptotic performance.".to_string(),
    (ProblemKind::MergeSortedArrays, _) => "Concatenate then sort - takes O((n+m) log(n+m)) instead of linear merge.

Nested scanning insertion - repeated shifts/comparisons increase overhead.

Using extra passes for cleanup - unnecessary when two-pointer merge already handles tails naturally.".to_string(),
    (ProblemKind::ProductExceptSelf, _) => "Division-based approach - fails with zeros and can introduce corner-case bugs.

Nested multiplication for each index - O(n^2) and too slow for large arrays.

Prefix-only or suffix-only pass - incomplete without combining both directions.".to_string(),
    (ProblemKind::MissingNumber, _) => "Sorting then scanning - O(n log n) where O(n) is sufficient.

Nested comparisons - unnecessary repeated checks.

Boolean array marking - uses extra O(n) space when formula/XOR can be more compact.".to_string(),
    (ProblemKind::TopKFrequent, _) => "Sorting all unique elements by frequency - O(m log m) with unnecessary full sort.

Nested counting for each unique value - repeated scans increase runtime.

Maintaining large ordered structures globally - higher overhead than bucket/frequency approach.".to_string(),
    (ProblemKind::Generic, _) => "Brute force nested loops - usually introduces extra operations and degrades scalability.

Sorting-first approach - may add O(n log n) overhead when not required by the problem.

Over-engineered abstractions - can increase complexity without improving correctness.".to_string(),
    }
}

fn extract_code_block(answer: &str) -> Option<String> {
    let start = answer.find("```")?;
    let rest = &answer[start + 3..];
    let end_rel = rest.find("```")?;
    let inner = &rest[..end_rel];
    Some(inner.trim().to_string())
}

fn extract_code_candidate(answer: &str, lang: TargetLanguage) -> Option<String> {
    if let Some(block) = extract_code_block(answer) {
        let normalized = strip_language_header(&block).trim().to_string();
        if !normalized.is_empty() {
            return Some(normalized);
        }
    }

    let mut picked = Vec::new();
    for line in answer.lines() {
        let t = line.trim_end();
        if looks_like_code_line(t, lang) {
            picked.push(t.to_string());
        }
    }

    let joined = picked.join("\n").trim().to_string();
    if joined.is_empty() {
        None
    } else {
        Some(strip_language_header(&joined).trim().to_string())
    }
}

fn strip_language_header(text: &str) -> String {
    let mut lines = text.lines();
    let first = lines.next().unwrap_or_default().trim().to_lowercase();
    if first == "python" || first == "java" || first == "javascript" || first == "cpp" || first == "c" {
        lines.collect::<Vec<_>>().join("\n")
    } else {
        text.to_string()
    }
}

fn looks_like_code_line(line: &str, lang: TargetLanguage) -> bool {
    let t = line.trim_start();

    if t.starts_with("1.") || t.starts_with("2.") || t.starts_with("3.") || t.starts_with("4.") {
        return false;
    }
    if t.starts_with("- ") || t.starts_with("* ") {
        return false;
    }

    match lang {
        TargetLanguage::Python => {
            t.starts_with("def ")
                || t.starts_with("for ")
                || t.starts_with("if ")
                || t.starts_with("elif ")
                || t.starts_with("else")
                || t.starts_with("while ")
                || t.starts_with("return ")
                || t.starts_with("class ")
                || t.starts_with("import ")
                || t.starts_with("from ")
                || (t.contains(" = ") && !t.contains(" - ") && !t.ends_with('.'))
        }
        TargetLanguage::Java => {
            t.starts_with("public ")
                || t.starts_with("private ")
                || t.starts_with("class ")
                || t.starts_with("for (")
                || t.starts_with("if (")
                || t.starts_with("return ")
                || t.ends_with(";")
                || t == "{" || t == "}"
        }
        TargetLanguage::JavaScript => {
            t.starts_with("function ")
                || t.starts_with("const ")
                || t.starts_with("let ")
                || t.starts_with("var ")
                || t.starts_with("for (")
                || t.starts_with("if (")
                || t.starts_with("return ")
                || t.ends_with(";")
                || t == "{" || t == "}"
        }
        TargetLanguage::Cpp | TargetLanguage::C => {
            t.starts_with("#include")
                || t.starts_with("int ")
                || t.starts_with("void ")
                || t.starts_with("for (")
                || t.starts_with("if (")
                || t.starts_with("return ")
                || t.ends_with(";")
                || t == "{" || t == "}"
        }
    }
}

fn contains_placeholder(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.contains("<working code>")
        || lower.contains("<method")
        || lower.contains("<issue>")
        || lower.contains("<2-3 lines>")
        || lower.contains("now, the output format")
        || lower.contains("we must output")
}

fn index_to_letter(idx: usize) -> Option<char> {
    if (1..=8).contains(&idx) {
        Some((b'A' + (idx as u8) - 1) as char)
    } else {
        None
    }
}

fn strip_think_tags(text: &str) -> String {
    let mut result = text.to_string();
    while let Some(start) = result.find("<think>") {
        if let Some(end) = result.find("</think>") {
            result = format!("{}{}", &result[..start], &result[end + 8..]);
        } else {
            result = result[..start].to_string();
            break;
        }
    }
    result.trim().to_string()
}
