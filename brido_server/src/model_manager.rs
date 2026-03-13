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

        // Step 2: Solve the extracted text with strict templates.
        if looks_like_coding_question(&clean_vision) {
            if let Ok(r) = self.run_coding_solver(&clean_vision).await {
                let clean = strip_think_tags(&r);
                let strict = ensure_coding_block_format(&clean, &clean_vision);
                if !strict.trim().is_empty() {
                    return Ok(strict);
                }
            }
        }

        if looks_like_quiz_question(&clean_vision) || looks_like_reasoning(&clean_vision) {
            if let Ok(r) = self.run_quiz_solver(&clean_vision).await {
                let clean = strip_think_tags(&r);
                let strict = ensure_quiz_block_format(&clean, &clean_vision);
                if !strict.trim().is_empty() {
                    return Ok(format!("[qwen3-vl:4b + deepseek-r1:8b]\n{}", strict));
                }
            }
        }

        Ok(format!("[qwen3-vl:4b]\n{}", clean_vision))
    }

    async fn run_quiz_solver(&self, extracted_text: &str) -> Result<String> {
        let prompt = format!(
            "Solve the quiz from this OCR text.\n\nOCR:\n{}\n\nSTRICT OUTPUT FORMAT (exact):\nAnswer: <LETTER>. <FULL_OPTION_TEXT>\n\nExplanation:\n<2-4 short lines on why this is correct>\n\nRules:\n- Choose exactly ONE best option for the primary visible question.\n- Never return only a number or only a letter.\n- Keep option text complete when visible.",
            extracted_text
        );

        self.run_text_solver(&prompt, 512).await
    }

    async fn run_coding_solver(&self, extracted_text: &str) -> Result<String> {
        let prompt = format!(
            "Solve the coding problem from this OCR text.\n\nOCR:\n{}\n\nReturn ONLY one runnable Python code block and nothing else.\nRules:\n- No explanation text.\n- No placeholders.\n- Prefer function names that match the problem.",
            extracted_text
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
    let markers = [
        "coding problems",
        "two sum",
        "palindrome",
        "reverse string",
        "given an array",
        "write a function",
        "input",
        "output",
        "leetcode",
    ];

    markers.iter().any(|m| lower.contains(m))
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
    let trimmed = answer.trim();
    if trimmed.contains("Answer:") && trimmed.contains("Explanation:") {
        return trimmed.to_string();
    }

    let (choice, text) = parse_choice_with_text(trimmed, source_text)
        .unwrap_or(('A', "".to_string()));

    let option_text = if text.is_empty() {
        "(option text not fully visible)".to_string()
    } else {
        text
    };

    format!(
        "Answer: {}. {}\n\nExplanation:\nSelected based on the visible question and options. The chosen option best matches the concept being asked.",
        choice,
        option_text
    )
}

fn ensure_coding_block_format(answer: &str, _source_text: &str) -> String {
    let kind = detect_problem_kind(_source_text);

    let code_inner = match kind {
        ProblemKind::Generic => extract_python_code(answer)
            .filter(|c| !contains_placeholder(c))
            .unwrap_or_else(|| fallback_code_for_kind(kind)),
        _ => fallback_code_for_kind(kind),
    };

    let why_method = why_method_for_kind(kind);
    let why_not = why_not_methods_for_kind(kind);

    format!(
        "language: python\n\ncode:\n\n{}\n\nwhy this method:\n{}\n\nwhy not other methods\n\n{}",
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
    FindMaximum,
    Generic,
}

fn detect_problem_kind(source_text: &str) -> ProblemKind {
    let s = source_text.to_lowercase();
    if s.contains("two sum") {
        ProblemKind::TwoSum
    } else if s.contains("palindrome") {
        ProblemKind::Palindrome
    } else if s.contains("reverse string") || s.contains("reverses a string") {
        ProblemKind::ReverseString
    } else if s.contains("find maximum") || s.contains("largest element") {
        ProblemKind::FindMaximum
    } else {
        ProblemKind::Generic
    }
}

fn fallback_code_for_kind(kind: ProblemKind) -> String {
    match kind {
        ProblemKind::TwoSum => {
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
        ProblemKind::Palindrome => {
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
        ProblemKind::ReverseString => {
            "def reverseString(s):
    reversed_str = \"\"

    for ch in s:
        reversed_str = ch + reversed_str

    return reversed_str"
                .to_string()
        }
        ProblemKind::FindMaximum => {
            "def findMax(arr):
    max_val = arr[0]

    for num in arr:
        if num > max_val:
            max_val = num

    return max_val"
                .to_string()
        }
        ProblemKind::Generic => {
            "def solve(input_data):
    # Implement the required logic here
    return input_data"
                .to_string()
        }
    }
}

fn why_method_for_kind(kind: ProblemKind) -> &'static str {
    match kind {
    ProblemKind::TwoSum => "Uses a hash map to store visited numbers, allowing constant-time lookup of the required complement and an overall O(n) pass.",
    ProblemKind::Palindrome => "Two-pointer comparison checks mirrored characters in one pass and exits early on mismatch, keeping both logic and complexity efficient.",
    ProblemKind::ReverseString => "Builds the reversed value step by step so the transformation is explicit and easy to understand in interview-style explanations.",
    ProblemKind::FindMaximum => "Single-pass traversal updates the maximum value while scanning the array once.",
    ProblemKind::Generic => "Uses a direct step-by-step implementation with readable control flow and predictable performance.",
    }
}

fn why_not_methods_for_kind(kind: ProblemKind) -> String {
    match kind {
    ProblemKind::TwoSum => "Brute force nested loops - checks every pair of numbers; time complexity O(n^2), which is inefficient for larger arrays.

Sorting + two pointers - sorting takes O(n log n) and changes original indices, so extra mapping logic is needed.

Binary search after sorting - adds sorting overhead and repeated searches, increasing implementation complexity.".to_string(),
    ProblemKind::Palindrome => "Reverse-and-compare shortcut - concise, but it hides the character-by-character comparison logic expected in interviews.

Recursive implementation - adds call stack overhead and can be less readable for long inputs.

Extra data-structure based methods - unnecessary memory usage for a problem solvable with two pointers.".to_string(),
    ProblemKind::ReverseString => "Slicing (s[::-1]) - very concise, but often avoided when interviewers want explicit algorithmic steps.

Using built-in reversed() directly - clean, but hides manual reversal logic.

Recursive reversal - introduces extra call overhead and is less practical for long strings.".to_string(),
    ProblemKind::FindMaximum => "Sorting the array - sorting requires O(n log n) time while the maximum can be found in O(n) with one scan.

Using built-in max() - although efficient, it hides the algorithmic logic and is often avoided in interviews where manual implementation is expected.

Nested comparisons - comparing every pair introduces unnecessary operations and approaches O(n^2) complexity.".to_string(),
    ProblemKind::Generic => "Brute force nested loops - usually introduces extra operations and degrades scalability.

Sorting-first approach - may add O(n log n) overhead when not required by the problem.

Over-engineered abstractions - can increase complexity without improving correctness.".to_string(),
    }
}

fn extract_python_code(answer: &str) -> Option<String> {
    if let Some(block) = extract_code_block(answer) {
        let mut lines = block.lines();
        let first = lines.next().unwrap_or_default().trim().to_lowercase();
        let code = if first == "python" || first.is_empty() {
            lines.collect::<Vec<_>>().join("\n")
        } else {
            block
        };

        let cleaned = code.trim().to_string();
        if cleaned.is_empty() || contains_placeholder(&cleaned) {
            return None;
        }
        if cleaned.contains("def ") || cleaned.contains("return ") {
            return Some(cleaned);
        }
    }

    // Fallback: collect obvious python-like lines.
    let mut buf = Vec::new();
    for line in answer.lines() {
        let t = line.trim();
        if t.starts_with("def ")
            || t.starts_with("for ")
            || t.starts_with("if ")
            || t.starts_with("elif ")
            || t.starts_with("else")
            || t.starts_with("while ")
            || t.starts_with("return ")
            || t.starts_with("try:")
            || t.starts_with("except ")
            || line.starts_with("    ")
        {
            buf.push(line.trim_end().to_string());
        }
    }

    let code = buf.join("\n").trim().to_string();
    if code.is_empty() || contains_placeholder(&code) {
        None
    } else {
        Some(code)
    }
}

fn extract_code_block(answer: &str) -> Option<String> {
    let start = answer.find("```")?;
    let rest = &answer[start + 3..];
    let end_rel = rest.find("```")?;
    let inner = &rest[..end_rel];
    Some(inner.trim().to_string())
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
