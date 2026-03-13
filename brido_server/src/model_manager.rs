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
}

const SYSTEM_PROMPT: &str = "\
You are an expert assistant answering questions visible on a laptop screenshot.

WHEN YOU SEE MCQ / QUIZ QUESTIONS:
- There may be multiple numbered questions. Answer ALL of them.
- Use your own knowledge to pick the CORRECT answer — do NOT guess from the image alone.
- Output one line per question in this exact format:
    1. B. Queue
    2. C. O(log n)
    3. B. HTML
- NEVER answer with only a numeric option index like 1/2/3/4. Convert it to letters (1=A, 2=B, 3=C, 4=D).
- ALWAYS include full option text after the letter when it is visible.
- If only part of an option is visible, answer with the letter and the visible part only.
- No explanations unless there is only one question.

WHEN YOU SEE A CODING PROBLEM:
- Write the solution code.
- Add a one-line explanation of the approach.

WHEN YOU SEE A MATH PROBLEM:
- Solve it. Show the answer and brief steps.

ANYTHING ELSE:
- Write 2-3 sentences describing what's on screen.

CRITICAL RULES:
- ALWAYS use your knowledge to verify the correct answer — never pick an option just because it appears first.
- READ every option carefully before deciding.
- Do NOT describe UI, windows, or layout.
- Keep responses SHORT and DIRECT.";

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

        let clean_vision = strip_think_tags(&vision_text);

        // Step 2: route only heavy reasoning content to DeepSeek.
        // Keeping this narrow improves reliability on low-memory systems.
        let needs_reasoning = looks_like_reasoning(&clean_vision);

        if needs_reasoning {
            let reasoning_result = self.run_reasoning(&clean_vision).await;
            match reasoning_result {
                Ok(r) if !r.trim().is_empty() => {
                    let clean = strip_think_tags(&r);
                    let normalized = normalize_answer_output(&clean, &clean_vision);
                    return Ok(format!("[qwen3-vl:4b + deepseek-r1:8b]\n{}", normalized));
                }
                _ => {} // fall through to vision-only result
            }
        }

        let normalized = normalize_answer_output(&clean_vision, &clean_vision);
        Ok(format!("[qwen3-vl:4b]\n{}", normalized))
    }

    async fn run_reasoning(&self, extracted_text: &str) -> Result<String> {
        let prompt = format!(
            "Solve this problem concisely:\n\n{}\n\nAnswer directly with solution and brief explanation.",
            extracted_text
        );

        let request = ChatRequest {
            model: "deepseek-r1:8b".to_string(),
            messages: vec![
                ChatMessage {
                    role: "user".to_string(),
                    content: prompt,
                    images: None,
                },
            ],
            stream: false,
            options: Some(ChatOptions {
                num_predict: 384,
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
            return Err(anyhow!("Ollama reasoning error {}: {}", status, body));
        }

        let response = response.json::<ChatResponse>().await?;

        Ok(response.message.content)
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
            Some(p) => format!("{}", p),
            None => format!("{}", SYSTEM_PROMPT),
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
                            Ok(body) => return Ok(body.message.content),
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

fn looks_like_reasoning(text: &str) -> bool {
    let lower = text.to_lowercase();
    let triggers = [
        "complexity", "algorithm", "function", "code", "program",
        "output", "implement", "recursion", "loop", "array",
        "prove", "derive", "calculate", "equation", "solve",
    ];
    triggers.iter().filter(|&&t| lower.contains(t)).count() >= 2
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

fn normalize_answer_output(text: &str, source_text: &str) -> String {
    let option_map = extract_option_map(source_text);
    let mut lines_out = Vec::new();

    for (idx, raw_line) in text.lines().enumerate() {
        let mut line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        if let Some(rest) = line.strip_prefix("Answer:") {
            line = rest.trim();
        } else if let Some(rest) = line.strip_prefix("answer:") {
            line = rest.trim();
        }

        let (question_no, body) = split_question_prefix(line, idx + 1);
        if let Some((choice, tail)) = parse_choice_prefix(&body) {
            let completed_tail = if tail.is_empty() {
                option_map
                    .get(&(question_no, choice))
                    .cloned()
                    .unwrap_or_default()
            } else {
                let from_map = option_map
                    .get(&(question_no, choice))
                    .cloned()
                    .unwrap_or_default();

                if from_map.is_empty() || tail.len() >= from_map.len().saturating_sub(3) {
                    tail
                } else {
                    from_map
                }
            };

            if completed_tail.is_empty() {
                lines_out.push(format!("{}. {}", question_no, choice));
            } else {
                lines_out.push(format!("{}. {}. {}", question_no, choice, completed_tail));
            }
        } else {
            lines_out.push(body);
        }
    }

    if lines_out.is_empty() {
        text.trim().to_string()
    } else {
        lines_out.join("\n")
    }
}

fn split_question_prefix(line: &str, fallback_no: usize) -> (usize, String) {
    let trimmed = line.trim();
    let mut chars = trimmed.chars().peekable();
    let mut digits = String::new();

    while let Some(c) = chars.peek().copied() {
        if c.is_ascii_digit() {
            digits.push(c);
            chars.next();
        } else {
            break;
        }
    }

    if digits.is_empty() {
        return (fallback_no, trimmed.to_string());
    }

    let mut has_sep = false;
    while let Some(c) = chars.peek().copied() {
        if c == '.' || c == ')' || c == ':' || c.is_whitespace() {
            has_sep = true;
            chars.next();
        } else {
            break;
        }
    }

    if !has_sep {
        return (fallback_no, trimmed.to_string());
    }

    let rest: String = chars.collect();
    let qn = digits.parse::<usize>().unwrap_or(fallback_no);
    (qn, rest.trim().to_string())
}

fn parse_choice_prefix(body: &str) -> Option<(char, String)> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return None;
    }

    let lower = trimmed.to_ascii_lowercase();
    let candidate = if let Some(rest) = lower.strip_prefix("option") {
        let original_rest = &trimmed[trimmed.len() - rest.len()..];
        original_rest.trim_start()
    } else {
        trimmed
    };

    let mut chars = candidate.chars();
    let first = chars.next()?;
    let mut tail = chars.as_str().trim_start();

    if tail.starts_with('.') || tail.starts_with(')') || tail.starts_with(':') {
        tail = tail[1..].trim_start();
    }

    if first.is_ascii_alphabetic() {
        let upper = first.to_ascii_uppercase();
        if ('A'..='H').contains(&upper) {
            return Some((upper, tail.to_string()));
        }
    }

    if first.is_ascii_digit() {
        let idx = first.to_digit(10)? as usize;
        if let Some(letter) = index_to_letter(idx) {
            return Some((letter, tail.to_string()));
        }
    }

    None
}

fn index_to_letter(idx: usize) -> Option<char> {
    if (1..=8).contains(&idx) {
        Some((b'A' + (idx as u8) - 1) as char)
    } else {
        None
    }
}

fn extract_option_map(source_text: &str) -> std::collections::HashMap<(usize, char), String> {
    let mut map = std::collections::HashMap::new();
    let mut current_question: Option<usize> = None;

    for raw_line in source_text.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        if let Some((q_no, _)) = parse_question_header(line) {
            current_question = Some(q_no);
            continue;
        }

        if let Some(q_no) = current_question {
            if let Some((choice, text)) = parse_option_line(line) {
                if !text.is_empty() {
                    map.insert((q_no, choice), text);
                }
            }
        }
    }

    map
}

fn parse_question_header(line: &str) -> Option<(usize, String)> {
    let mut chars = line.chars().peekable();
    let mut digits = String::new();

    while let Some(c) = chars.peek().copied() {
        if c.is_ascii_digit() {
            digits.push(c);
            chars.next();
        } else {
            break;
        }
    }

    if digits.is_empty() {
        return None;
    }

    let sep = chars.peek().copied()?;
    if sep != '.' && sep != ')' {
        return None;
    }
    chars.next();

    let rest: String = chars.collect();
    let q_no = digits.parse::<usize>().ok()?;
    Some((q_no, rest.trim().to_string()))
}

fn parse_option_line(line: &str) -> Option<(char, String)> {
    let mut chars = line.chars();
    let first = chars.next()?;
    let choice = first.to_ascii_uppercase();
    if !('A'..='H').contains(&choice) {
        return None;
    }

    let rest = chars.as_str().trim_start();
    let rest = if let Some(r) = rest.strip_prefix('.') {
        r.trim_start()
    } else if let Some(r) = rest.strip_prefix(')') {
        r.trim_start()
    } else if let Some(r) = rest.strip_prefix(':') {
        r.trim_start()
    } else {
        return None;
    };

    Some((choice, rest.to_string()))
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
