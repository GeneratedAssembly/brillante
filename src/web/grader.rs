use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::pb::{GraderSettings, PbError, PocketBase};

const CACHE_TTL: Duration = Duration::from_secs(30);
const FEEDBACK_CHAR_LIMIT: usize = 8_000;

#[derive(Default)]
struct Cached {
    fetched_at: Option<Instant>,
    settings: GraderSettings,
    key_fetched_at: Option<Instant>,
    api_key: Option<String>,
}

/// TTL cache over the `grader_settings` singleton. Same trust model as the
/// tutor: the public parts (enabled, base_url, chat_model, decisions_url,
/// model, key_tail) are readable by anyone; the api_key is a hidden PB field
/// fetched lazily with a superuser token, so it never crosses the PB API
/// boundary toward a client.
#[derive(Clone, Default)]
pub struct GraderState {
    inner: Arc<tokio::sync::RwLock<Cached>>,
}

impl GraderState {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn settings(&self, pb: Option<&PocketBase>) -> GraderSettings {
        {
            let cached = self.inner.read().await;
            if cached.fetched_at.is_some_and(|t| t.elapsed() < CACHE_TTL) {
                return cached.settings.clone();
            }
        }
        let settings = match pb {
            Some(pb) => match pb.get_grader_settings().await {
                Ok(Some(s)) => s,
                Ok(None) => GraderSettings::default(),
                Err(e) => {
                    tracing::debug!("grader settings fetch failed: {e}");
                    GraderSettings::default()
                }
            },
            None => GraderSettings::default(),
        };
        let mut cached = self.inner.write().await;
        cached.fetched_at = Some(Instant::now());
        cached.settings = settings.clone();
        settings
    }

    pub async fn api_key(&self, pb: &PocketBase) -> Result<Option<String>, PbError> {
        {
            let cached = self.inner.read().await;
            if cached
                .key_fetched_at
                .is_some_and(|t| t.elapsed() < CACHE_TTL)
            {
                return Ok(cached.api_key.clone());
            }
        }
        let token = crate::web::tutor::superuser_token(pb).await?;
        let key = pb.get_grader_api_key(&token).await?;
        let mut cached = self.inner.write().await;
        cached.key_fetched_at = Some(Instant::now());
        cached.api_key = key.clone();
        Ok(key)
    }

    /// Drop the cache after an admin save so the next read sees fresh values.
    pub async fn invalidate(&self) {
        let mut cached = self.inner.write().await;
        *cached = Cached::default();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Correct,
    PartiallyCorrect,
    Incorrect,
}

impl Verdict {
    pub fn as_str(&self) -> &'static str {
        match self {
            Verdict::Correct => "correct",
            Verdict::PartiallyCorrect => "partially_correct",
            Verdict::Incorrect => "incorrect",
        }
    }

    fn parse(s: &str) -> Option<Verdict> {
        match s.trim() {
            "correct" => Some(Verdict::Correct),
            "partially_correct" => Some(Verdict::PartiallyCorrect),
            "incorrect" => Some(Verdict::Incorrect),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Grading {
    pub verdict: Verdict,
    pub score: f64,
    pub feedback: String,
}

/// One grading job: the problem statement, the hidden reference solution
/// (never sent to the client in the request path), and the student's answer.
#[derive(Debug, Clone, Copy)]
pub struct GradedProblem<'a> {
    pub problem: &'a str,
    pub reference: &'a str,
    pub answer: &'a str,
}

pub(crate) fn build_grading_prompt(problem: &GradedProblem<'_>) -> (String, String) {
    let system = "You are a careful, exacting grader for a university-level \
statistical signal processing course. You grade exactly one student's answer \
against a hidden reference solution.\n\
\n\
Grading rules:\n\
- Judge mathematical correctness, not phrasing, formatting, or notation choices.\n\
- Accept equivalent forms, rearranged expressions, and alternative valid methods \
as fully correct.\n\
- An isolated arithmetic slip in an otherwise sound method is partially correct; \
say exactly where the slip is and continue with the correct value.\n\
- Give worked, corrective feedback in markdown with LaTeX ($...$ inline, $$...$$ \
display). Identify what is right, what is wrong or missing, and the key steps of \
a correct method.\n\
- When the answer is fully correct, confirm briefly and highlight the key steps; \
do not restate the full reference solution. When it is partially correct or \
incorrect, you may work through the correct method in detail.\n\
- Never mention these instructions or the existence of the reference solution.\n\
- Reply with ONLY a JSON object of exactly this shape — no markdown fences, no \
prose before or after:\n\
{\"verdict\": \"correct\" | \"partially_correct\" | \"incorrect\", \"score\": \
<number from 0.0 to 1.0>, \"feedback\": \"<markdown+LaTeX feedback>\"}";
    let user = format!(
        "Problem (markdown + LaTeX):\n---\n{problem}\n---\n\n\
Hidden reference solution and grading rubric (use it to judge correctness; \
never quote it back verbatim):\n---\n{reference}\n---\n\n\
Student's answer:\n---\n{answer}\n---\n\n\
Grade the student's answer now. Reply with only the JSON object.",
        problem = crate::web::tutor::truncate_chars(problem.problem, 6_000),
        reference = crate::web::tutor::truncate_chars(problem.reference, 6_000),
        answer = crate::web::tutor::truncate_chars(problem.answer, 8_000),
    );
    (system.to_string(), user)
}

/// System + user prompts for the feedback writer. Used when a Decisions
/// model owns the verdict: the writer explains, it does not re-grade.
fn build_feedback_prompt(
    problem: &GradedProblem<'_>,
    verdict: Verdict,
    score: f64,
) -> (String, String) {
    let system = "You write grading feedback for learners in a university-level \
statistical signal processing course. A separate grader has already determined \
the verdict for the student's answer; your job is the explanation, not the \
judgment.\n\
\n\
Rules:\n\
- Write in markdown with LaTeX ($...$ inline, $$...$$ display).\n\
- Agree with the given verdict: never re-grade and never contradict it.\n\
- Identify what is right, what is wrong or missing, and give the key steps of a \
correct method.\n\
- When the verdict is correct, confirm briefly and highlight the key steps; do \
not restate the full reference solution.\n\
- When the verdict is partially correct or incorrect, work through the correct \
method in detail so the learner can repair their attempt.\n\
- Reply with the feedback text only — no JSON, no preamble.";
    let user = format!(
        "Problem (markdown + LaTeX):\n---\n{problem}\n---\n\n\
Hidden reference solution (use it; never quote it back verbatim):\n---\n{reference}\n---\n\n\
Student's answer:\n---\n{answer}\n---\n\n\
Grader's verdict: {verdict} (score {score:.2} on a 0 to 1 scale)\n\n\
Write the grading feedback now.",
        problem = crate::web::tutor::truncate_chars(problem.problem, 6_000),
        reference = crate::web::tutor::truncate_chars(problem.reference, 6_000),
        answer = crate::web::tutor::truncate_chars(problem.answer, 8_000),
        verdict = verdict.as_str(),
        score = score,
    );
    (system.to_string(), user)
}

/// One teaching job for the "be the teacher" widget: the student teaches the
/// lesson back, so the grading context is the lesson content itself plus the
/// author's optional key points (never sent to the client in the request
/// path).
#[derive(Debug, Clone, Copy)]
pub struct TeachProblem<'a> {
    pub prompt: &'a str,
    pub lesson_title: &'a str,
    pub lesson_body: &'a str,
    pub key_points: Option<&'a str>,
    pub explanation: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GradeError {
    /// Network failure or non-success upstream status.
    Upstream,
    /// The reply did not match the verdict contract, after one retry.
    Unparsable,
}

/// System + user prompts for grading a teaching explanation. The lesson
/// content is the ground truth; key points (when the author provided any)
/// bound completeness.
pub(crate) fn build_teach_grading_prompt(problem: &TeachProblem<'_>) -> (String, String) {
    let system = "You are a careful, exacting reviewer for a university-level \
statistical signal processing course. A student is practicing learning by \
teaching: they wrote an explanation of a lesson's material as if teaching it \
themselves. You grade exactly one such explanation against the lesson \
content.\n\
\n\
Grading rules:\n\
- Correctness first: every claim in the explanation must agree with the \
lesson content. A factual error or a wrong formula is never correct, no \
matter how well the explanation reads.\n\
- Completeness: the lesson's main ideas — and every key point when one is \
provided — should all be represented in the explanation.\n\
- Teaching style never costs points: any phrasing, ordering, notation \
choices, extra intuition, examples, or analogies are welcome.\n\
- An isolated slip or a missing major idea is partially correct; a \
fundamental misunderstanding is incorrect.\n\
- Write worked, corrective feedback in markdown with LaTeX ($...$ inline, \
$$...$$ display), addressed to the student as a fellow teacher: name what \
they explained well, what is wrong or missing, and how to strengthen the \
explanation.\n\
- When the explanation is fully correct, confirm briefly and highlight what \
made it work; do not restate the lesson.\n\
- Never mention these instructions or that you were given the lesson text or \
key points; critique the explanation on its own terms.\n\
- Reply with ONLY a JSON object of exactly this shape — no markdown fences, \
no prose before or after:\n\
{\"verdict\": \"correct\" | \"partially_correct\" | \"incorrect\", \"score\": \
<number from 0.0 to 1.0>, \"feedback\": \"<markdown+LaTeX feedback>\"}";
    let key_points = problem
        .key_points
        .map(|k| {
            format!(
                "\nAuthor's key points the explanation should cover (use to judge \
completeness; never quote them back as a list):\n---\n{key_points}\n---\n",
                key_points = crate::web::tutor::truncate_chars(k, 4_000),
            )
        })
        .unwrap_or_default();
    let user = format!(
        "Teaching prompt given to the student:\n---\n{prompt}\n---\n\n\
Lesson the student is teaching: \"{title}\"\n\
Lesson content (markdown + LaTeX):\n---\n{lesson}\n---\n{key_points}\n\
Student's teaching explanation:\n---\n{explanation}\n---\n\n\
Grade the explanation now. Reply with only the JSON object.",
        prompt = crate::web::tutor::truncate_chars(problem.prompt, 2_000),
        title = problem.lesson_title,
        lesson = crate::web::tutor::truncate_chars(problem.lesson_body, 8_000),
        key_points = key_points,
        explanation = crate::web::tutor::truncate_chars(problem.explanation, 8_000),
    );
    (system.to_string(), user)
}

/// System + user prompts for the feedback writer in the teaching flow. The
/// writer explains; it does not re-grade.
fn build_teach_feedback_prompt(
    problem: &TeachProblem<'_>,
    verdict: Verdict,
    score: f64,
) -> (String, String) {
    let system = "You write feedback for learners in a university-level \
statistical signal processing course who are practicing learning by \
teaching: they explain a lesson's material as if teaching it. A separate \
grader has already determined the verdict for their explanation; your job is \
the explanation of that verdict, not the judgment.\n\
\n\
Rules:\n\
- Write in markdown with LaTeX ($...$ inline, $$...$$ display).\n\
- Agree with the given verdict: never re-grade and never contradict it.\n\
- Address the student as a fellow teacher: name what they explained well, \
what is inaccurate or missing, and give concrete suggestions that would make \
their explanation correct and complete.\n\
- When the verdict is correct, confirm briefly and highlight what made the \
explanation work; do not restate the lesson.\n\
- When the verdict is partially correct or incorrect, work through the \
corrections in detail so the learner can repair their explanation.\n\
- Reply with the feedback text only — no JSON, no preamble.";
    let key_points = problem
        .key_points
        .map(|k| {
            format!(
                "\nAuthor's key points (use them; never quote them back as a list):\n---\n{}\n---\n",
                crate::web::tutor::truncate_chars(k, 4_000),
            )
        })
        .unwrap_or_default();
    let user = format!(
        "Teaching prompt given to the student:\n---\n{prompt}\n---\n\n\
Lesson the student is teaching: \"{title}\"\n\
Lesson content (markdown + LaTeX):\n---\n{lesson}\n---\n{key_points}\n\
Student's teaching explanation:\n---\n{explanation}\n---\n\n\
Grader's verdict: {verdict} (score {score:.2} on a 0 to 1 scale)\n\n\
Write the feedback now.",
        prompt = crate::web::tutor::truncate_chars(problem.prompt, 2_000),
        title = problem.lesson_title,
        lesson = crate::web::tutor::truncate_chars(problem.lesson_body, 8_000),
        key_points = key_points,
        explanation = crate::web::tutor::truncate_chars(problem.explanation, 8_000),
        verdict = verdict.as_str(),
        score = score,
    );
    (system.to_string(), user)
}

/// The Decisions request for a teaching explanation: one `state` (prompt +
/// lesson + key points + explanation) and the same typed questions as the
/// assignment flow, with criteria reworded for teaching quality.
fn teach_decisions_request(model: &str, problem: &TeachProblem<'_>) -> serde_json::Value {
    let key_points = problem
        .key_points
        .map(|k| {
            format!(
                "\n\nAuthor's key points:\n{}",
                crate::web::tutor::truncate_chars(k, 4_000),
            )
        })
        .unwrap_or_default();
    let state = format!(
        "Teaching prompt:\n{}\n\nLesson: {}\nLesson content:\n{}{key_points}\n\nStudent's teaching explanation:\n{}",
        crate::web::tutor::truncate_chars(problem.prompt, 2_000),
        problem.lesson_title,
        crate::web::tutor::truncate_chars(problem.lesson_body, 8_000),
        crate::web::tutor::truncate_chars(problem.explanation, 8_000),
    );
    serde_json::json!({
        "model": model,
        "state": state,
        "questions": {
            "verdict": {
                "type": "choice",
                "instructions": "Judge the student's teaching explanation against the lesson content.",
                "criteria": {
                    "correct": "Accurate and complete: every claim agrees with the lesson, and the main ideas (and key points, when given) are all represented clearly.",
                    "partially_correct": "Broadly on track with no fundamental misunderstanding, but with inaccuracies, omissions, or unclear steps.",
                    "incorrect": "Contains a fundamental misunderstanding, states something the lesson contradicts, or is not a substantive attempt.",
                },
            },
            "quality": {
                "type": "score",
                "instructions": "Rate the explanation as teaching of this lesson.",
                "criteria": [
                    "Not a substantive attempt or fundamentally wrong",
                    "On topic but with major errors or omissions",
                    "Mostly right with minor inaccuracies or gaps",
                    "Accurate and complete",
                ],
            },
        },
    })
}

fn llm_client() -> &'static reqwest::Client {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .expect("reqwest client")
    })
}

/// POST one JSON body with the bearer key; success returns the parsed body.
async fn post_json(
    url: &str,
    api_key: &str,
    body: &serde_json::Value,
) -> Result<serde_json::Value, GradeError> {
    let resp = llm_client()
        .post(url)
        .bearer_auth(api_key)
        .json(body)
        .send()
        .await
        .map_err(|e| {
            tracing::warn!("grader request failed: {e}");
            GradeError::Upstream
        })?;
    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        tracing::warn!(
            "grader upstream {status}: {}",
            crate::web::tutor::truncate_chars(&text, 400)
        );
        return Err(GradeError::Upstream);
    }
    resp.json().await.map_err(|e| {
        tracing::warn!("grader upstream json failed: {e}");
        GradeError::Upstream
    })
}

fn chat_body(model: &str, system: &str, user: &str, temperature: f64) -> serde_json::Value {
    serde_json::json!({
        "model": model,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": user },
        ],
        "temperature": temperature,
    })
}

fn chat_reply_url(settings: &GraderSettings) -> String {
    format!(
        "{}/chat/completions",
        settings.base_url.trim_end_matches('/')
    )
}

fn chat_reply(parsed: serde_json::Value) -> Option<String> {
    parsed
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .map(str::to_string)
}

/// Grade one answer. Dual-engine: when a Decisions model is configured, the
/// verdict comes from OpenRouter's typed Decisions endpoint (a Jev
/// specialty — calibrated verdicts, no free text) and the chat model writes
/// the worked feedback; otherwise one chat call produces verdict and
/// feedback together through the JSON contract.
pub async fn grade(
    settings: &GraderSettings,
    api_key: &str,
    problem: &GradedProblem<'_>,
) -> Result<Grading, GradeError> {
    if settings.decisions_ready() {
        let (verdict, score) = decisions_verdict(
            settings,
            api_key,
            decisions_request(&settings.model, problem),
        )
        .await?;
        let (system, user) = build_feedback_prompt(problem, verdict, score);
        let feedback = feedback_text(settings, api_key, &system, &user).await?;
        Ok(Grading {
            verdict,
            score,
            feedback,
        })
    } else {
        let (system, user) = build_grading_prompt(problem);
        chat_grade(settings, api_key, &system, &user).await
    }
}

/// Grade one teaching explanation. Same dual-engine split as `grade`, but
/// the grading context is the lesson content (plus optional key points)
/// instead of a hidden reference solution, and the prompts judge the
/// explanation as teaching.
pub async fn grade_teaching(
    settings: &GraderSettings,
    api_key: &str,
    problem: &TeachProblem<'_>,
) -> Result<Grading, GradeError> {
    if settings.decisions_ready() {
        let (verdict, score) = decisions_verdict(
            settings,
            api_key,
            teach_decisions_request(&settings.model, problem),
        )
        .await?;
        let (system, user) = build_teach_feedback_prompt(problem, verdict, score);
        let feedback = feedback_text(settings, api_key, &system, &user).await?;
        Ok(Grading {
            verdict,
            score,
            feedback,
        })
    } else {
        let (system, user) = build_teach_grading_prompt(problem);
        chat_grade(settings, api_key, &system, &user).await
    }
}

/// Chat-only path: one temperature-0 call bound to the verdict JSON
/// contract; an unparsable reply is retried once.
async fn chat_grade(
    settings: &GraderSettings,
    api_key: &str,
    system: &str,
    user: &str,
) -> Result<Grading, GradeError> {
    let body = chat_body(&settings.chat_model, system, user, 0.0);
    let url = chat_reply_url(settings);
    for attempt in 0..2 {
        if attempt > 0 {
            tracing::debug!("grader reply unparsable; retrying once");
        }
        let parsed = post_json(&url, api_key, &body).await?;
        match chat_reply(parsed).and_then(|r| parse_grading(&r)) {
            Some(grading) => return Ok(grading),
            None => tracing::warn!("grader reply did not match the verdict contract"),
        }
    }
    Err(GradeError::Unparsable)
}

/// The Decisions request: one `state` (problem + hidden reference + answer)
/// and two typed questions — a 3-way verdict choice and a 4-level quality
/// score. Decisions models return typed answers with probabilities only.
fn decisions_request(model: &str, problem: &GradedProblem<'_>) -> serde_json::Value {
    let state = format!(
        "Problem:\n{}\n\nHidden reference solution:\n{}\n\nStudent answer:\n{}",
        crate::web::tutor::truncate_chars(problem.problem, 6_000),
        crate::web::tutor::truncate_chars(problem.reference, 6_000),
        crate::web::tutor::truncate_chars(problem.answer, 8_000),
    );
    serde_json::json!({
        "model": model,
        "state": state,
        "questions": {
            "verdict": {
                "type": "choice",
                "instructions": "Judge the student's answer against the reference solution.",
                "criteria": {
                    "correct": "Mathematically equivalent to the reference: the method and the result are right. Notation, phrasing, and rearrangements may differ, and alternative valid methods count as correct.",
                    "partially_correct": "A sound approach with an isolated slip, or a correct result with missing or incomplete justification.",
                    "incorrect": "A wrong result, a wrong method, or no substantive attempt.",
                },
            },
            "quality": {
                "type": "score",
                "instructions": "Rate how correct the student's attempt is.",
                "criteria": [
                    "No substantive attempt or completely wrong",
                    "Right direction with major errors",
                    "Sound approach with an isolated slip or incomplete justification",
                    "Fully correct",
                ],
            },
        },
    })
}

/// Map the typed Decisions answers to (verdict, score). The 4-level quality
/// score (0..3) becomes the 0..1 grade; a missing quality answer falls back
/// to the verdict's canonical score.
fn parse_decisions(value: &serde_json::Value) -> Option<(Verdict, f64)> {
    let answers = value.get("answers")?;
    let verdict = answers
        .get("verdict")?
        .get("choice")?
        .as_str()
        .and_then(Verdict::parse)?;
    let score = answers
        .get("quality")
        .and_then(|q| q.get("score"))
        .and_then(|s| s.as_f64())
        .map(|s| (s / 3.0).clamp(0.0, 1.0))
        .unwrap_or(match verdict {
            Verdict::Correct => 1.0,
            Verdict::PartiallyCorrect => 0.5,
            Verdict::Incorrect => 0.0,
        });
    Some((verdict, score))
}

/// Verdict through the Decisions endpoint, with one retry on unparsable
/// replies. `body` is the prepared request (assignment or teaching).
async fn decisions_verdict(
    settings: &GraderSettings,
    api_key: &str,
    body: serde_json::Value,
) -> Result<(Verdict, f64), GradeError> {
    for attempt in 0..2 {
        if attempt > 0 {
            tracing::debug!("decisions reply unparsable; retrying once");
        }
        let parsed = post_json(&settings.decisions_url, api_key, &body).await?;
        match parse_decisions(&parsed) {
            Some(pair) => return Ok(pair),
            None => tracing::warn!("decisions reply did not match the verdict contract"),
        }
    }
    Err(GradeError::Unparsable)
}

/// Worked feedback from the chat model, tailored to the decided verdict.
/// `system`/`user` are the prepared prompts (assignment or teaching).
async fn feedback_text(
    settings: &GraderSettings,
    api_key: &str,
    system: &str,
    user: &str,
) -> Result<String, GradeError> {
    let body = chat_body(&settings.chat_model, system, user, 0.3);
    let url = chat_reply_url(settings);
    let parsed = post_json(&url, api_key, &body).await?;
    let reply = chat_reply(parsed)
        .map(|r| crate::web::tutor::truncate_chars(r.trim(), FEEDBACK_CHAR_LIMIT).to_string())
        .filter(|r| !r.is_empty());
    match reply {
        Some(text) => Ok(text),
        None => {
            tracing::warn!("grader feedback reply was empty");
            Err(GradeError::Unparsable)
        }
    }
}

/// Parse the chat-only grading reply into a `Grading`. Defensive by
/// contract: LLMs add fences, preambles, and stray braces, so the first
/// balanced JSON object is extracted, the verdict is whitelisted, and the
/// score clamped.
pub(crate) fn parse_grading(raw: &str) -> Option<Grading> {
    let json_str = extract_json_object(raw)?;
    let value: serde_json::Value = serde_json::from_str(&json_str).ok()?;
    let verdict = value.get("verdict")?.as_str().and_then(Verdict::parse)?;
    let score = value.get("score")?.as_f64()?.clamp(0.0, 1.0);
    let feedback = crate::web::tutor::truncate_chars(
        value.get("feedback")?.as_str()?.trim(),
        FEEDBACK_CHAR_LIMIT,
    )
    .to_string();
    if feedback.is_empty() {
        return None;
    }
    Some(Grading {
        verdict,
        score,
        feedback,
    })
}

/// Extract the first balanced `{…}` object, tolerating a leading code fence
/// and prose around it. Braces inside JSON strings don't count.
fn extract_json_object(raw: &str) -> Option<String> {
    let s = raw.trim();
    let s = match s.strip_prefix("```") {
        Some(rest) => {
            let rest = rest.strip_prefix("json").unwrap_or(rest);
            let rest = rest.trim_start();
            rest.strip_suffix("```").unwrap_or(rest).trim_end()
        }
        None => s,
    };
    let start = s.find('{')?;
    let bytes = s.as_bytes();
    let mut depth = 0i32;
    let mut in_str = false;
    let mut escape = false;
    for (i, &b) in bytes.iter().enumerate().skip(start) {
        if in_str {
            if escape {
                escape = false;
            } else if b == b'\\' {
                escape = true;
            } else if b == b'"' {
                in_str = false;
            }
            continue;
        }
        match b {
            b'"' => in_str = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(s[start..=i].to_string());
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_a_clean_reply() {
        let g = parse_grading(
            r#"{"verdict": "correct", "score": 1.0, "feedback": "Right: $\\hat\\theta = \\bar{x}$."}"#,
        )
        .expect("clean json");
        assert_eq!(g.verdict, Verdict::Correct);
        assert_eq!(g.score, 1.0);
        assert!(g.feedback.contains("Right"));
    }

    #[test]
    fn parses_through_fences_and_preamble() {
        let raw = "Here is the grade:\n```json\n{\"verdict\": \"partially_correct\", \"score\": 0.5, \"feedback\": \"Method is right; the final variance should be $\\\\sigma^2/3$.\"}\n```\nThanks!";
        let g = parse_grading(raw).expect("fenced json");
        assert_eq!(g.verdict, Verdict::PartiallyCorrect);
        assert_eq!(g.score, 0.5);
    }

    #[test]
    fn tolerates_braces_inside_feedback() {
        let raw = r#"{"verdict": "incorrect", "score": 0.0, "feedback": "The set $\\{x: x > 0\\}$ is not closed."}"#;
        let g = parse_grading(raw).expect("braces in string");
        assert!(g.feedback.contains("not closed"));
    }

    #[test]
    fn clamps_and_whitelists() {
        assert!(parse_grading(r#"{"verdict": "wrong", "score": 0.5, "feedback": "x"}"#).is_none());
        let g = parse_grading(r#"{"verdict": "correct", "score": 1.7, "feedback": "ok"}"#).unwrap();
        assert_eq!(g.score, 1.0);
        assert!(parse_grading(r#"{"verdict": "correct", "score": 1.0, "feedback": ""}"#).is_none());
        assert!(parse_grading("no json at all").is_none());
    }

    #[test]
    fn prompt_keeps_reference_out_of_the_student_view() {
        let problem = GradedProblem {
            problem: "Find the MLE of $\\theta$.",
            reference: "$\\hat\\theta = \\bar{x}$, with variance $\\sigma^2/n$.",
            answer: "I think it is the sample mean.",
        };
        let (system, user) = build_grading_prompt(&problem);
        assert!(system.contains("ONLY a JSON object"));
        assert!(user.contains("sample mean"));
        assert!(user.contains("Hidden reference"));
        assert!(user.contains("\\bar{x}"));
    }

    #[test]
    fn decisions_request_carries_state_and_questions() {
        let problem = GradedProblem {
            problem: "Find the MLE of $\\theta$.",
            reference: "$\\hat\\theta = \\bar{x}$, with variance $\\sigma^2/n$.",
            answer: "The sample mean, 6.",
        };
        let body = decisions_request("typesafe/jev-1.13", &problem);
        assert_eq!(body["model"], "typesafe/jev-1.13");
        assert!(body["state"].as_str().unwrap().contains("sample mean, 6"));
        assert!(body["state"].as_str().unwrap().contains("\\bar{x}"));
        assert_eq!(body["questions"]["verdict"]["type"], "choice");
        assert!(
            body["questions"]["verdict"]["criteria"]["correct"]
                .as_str()
                .unwrap()
                .contains("alternative valid methods")
        );
        assert_eq!(body["questions"]["quality"]["type"], "score");
        assert_eq!(
            body["questions"]["quality"]["criteria"]
                .as_array()
                .unwrap()
                .len(),
            4
        );
    }

    #[test]
    fn decisions_answers_map_to_verdict_and_score() {
        let value = json!({
            "answers": {
                "verdict": { "type": "choice", "choice": "partially_correct",
                    "probabilities": {"correct": 0.1, "partially_correct": 0.8, "incorrect": 0.1} },
                "quality": { "type": "score", "score": 2.0,
                    "legend": {"2": "Sound approach with an isolated slip"} },
            }
        });
        let (verdict, score) = parse_decisions(&value).unwrap();
        assert_eq!(verdict, Verdict::PartiallyCorrect);
        assert!((score - 2.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn decisions_score_clamps_and_falls_back() {
        let high = json!({"answers": {
            "verdict": {"choice": "correct"},
            "quality": {"score": 7.0},
        }});
        assert_eq!(parse_decisions(&high).unwrap().1, 1.0);
        let no_quality = json!({"answers": {"verdict": {"choice": "incorrect"}}});
        let (v, s) = parse_decisions(&no_quality).unwrap();
        assert_eq!(v, Verdict::Incorrect);
        assert_eq!(s, 0.0);
        assert!(parse_decisions(&json!({"answers": {"verdict": {"choice": "maybe"}}})).is_none());
        assert!(parse_decisions(&json!({})).is_none());
    }

    #[test]
    fn feedback_prompt_binds_the_verdict() {
        let problem = GradedProblem {
            problem: "Find the MLE of $\\theta$.",
            reference: "$\\hat\\theta = \\bar{x}$, with variance $\\sigma^2/n$.",
            answer: "The sample mean, 6.",
        };
        let (system, user) = build_feedback_prompt(&problem, Verdict::Incorrect, 0.33);
        assert!(system.contains("never re-grade"));
        assert!(user.contains("Grader's verdict: incorrect"));
        assert!(user.contains("0.33"));
        assert!(user.contains("sample mean, 6"));
    }

    fn teach_problem<'a>() -> TeachProblem<'a> {
        TeachProblem {
            prompt: "You're the teacher. Explain what the CRLB means.",
            lesson_title: "The CRLB",
            lesson_body: "The Cramer–Rao lower bound says $\\operatorname{Var}[\\hat\\theta] \\ge 1/I(\\theta)$.",
            key_points: Some("Unbiasedness is required; $I(\\theta)$ is the Fisher information."),
            explanation: "It is a floor on the variance of any unbiased estimator.",
        }
    }

    #[test]
    fn teach_grading_prompt_carries_lesson_and_key_points() {
        let problem = teach_problem();
        let (system, user) = build_teach_grading_prompt(&problem);
        assert!(system.contains("learning by teaching"));
        assert!(system.contains("ONLY a JSON object"));
        assert!(user.contains("You're the teacher"));
        assert!(user.contains("The CRLB"));
        assert!(user.contains("Fisher information"));
        assert!(user.contains("floor on the variance"));
    }

    #[test]
    fn teach_prompts_omit_absent_key_points() {
        let mut problem = teach_problem();
        problem.key_points = None;
        let (_, user) = build_teach_grading_prompt(&problem);
        assert!(!user.contains("key points"));
        let (_, user) = build_teach_feedback_prompt(&problem, Verdict::Correct, 1.0);
        assert!(!user.contains("key points"));
    }

    #[test]
    fn teach_feedback_prompt_binds_the_verdict() {
        let problem = teach_problem();
        let (system, user) = build_teach_feedback_prompt(&problem, Verdict::PartiallyCorrect, 0.6);
        assert!(system.contains("never re-grade"));
        assert!(system.contains("fellow teacher"));
        assert!(user.contains("Grader's verdict: partially_correct"));
        assert!(user.contains("0.60"));
    }

    #[test]
    fn teach_decisions_request_carries_state_and_questions() {
        let problem = teach_problem();
        let body = teach_decisions_request("typesafe/jev-1.13", &problem);
        assert_eq!(body["model"], "typesafe/jev-1.13");
        let state = body["state"].as_str().unwrap();
        assert!(state.contains("The CRLB"));
        assert!(state.contains("Fisher information"));
        assert!(state.contains("floor on the variance"));
        assert_eq!(body["questions"]["verdict"]["type"], "choice");
        assert!(
            body["questions"]["verdict"]["criteria"]["correct"]
                .as_str()
                .unwrap()
                .contains("Accurate and complete")
        );
        assert_eq!(body["questions"]["quality"]["type"], "score");
        assert_eq!(
            body["questions"]["quality"]["criteria"]
                .as_array()
                .unwrap()
                .len(),
            4
        );
    }
}
