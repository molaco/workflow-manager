# Testing Approaches for Interactive Educational Rust Applications

## Executive Summary

This research document covers comprehensive testing strategies for interactive Japanese learning applications built with Rust, focusing on character recognition accuracy, learning flow validation, stroke order algorithms, GUI component testing in iced, and learning effectiveness metrics.

---

## 1. Unit Testing for Character Recognition Accuracy

### Key Findings

**State-of-the-art Accuracy (2025)**
- Advanced models (GPT-4o, Claude 3.7 Sonnet): 82–90% accuracy for cursive handwriting
- Traditional OCR: 50–70% for cursive text
- Japanese character recognition: 98.5–99.17% accuracy for stroke order validation
- Printed text consistently outperforms handwriting recognition by 5–15%

### Recommended Rust Libraries

**1. OAROCR**
- Comprehensive OCR library with ONNX Runtime integration
- Provides confidence scores for extracted text
- Pre-trained models for various OCR tasks
- Efficient inference pipeline

**2. ocrs**
- Modern Rust OCR library with extensive ML integration
- Uses PyTorch-trained models exported to ONNX
- CLI and library interface
- Well-suited for testing pipelines

### Testing Strategies

#### Confidence Score Validation
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_character_recognition_confidence() {
        let recognized = recognize_character("test_images/hiragana_a.png");

        // Assert minimum confidence threshold
        assert!(recognized.confidence >= 0.85,
            "Recognition confidence too low: {}", recognized.confidence);

        // Assert correct character
        assert_eq!(recognized.character, "あ");
    }

    #[test]
    fn test_recognition_accuracy_batch() {
        let test_set = load_labeled_test_set("test_data/hiragana/");
        let mut correct = 0;

        for sample in test_set {
            let result = recognize_character(&sample.image_path);
            if result.character == sample.label {
                correct += 1;
            }
        }

        let accuracy = (correct as f64 / test_set.len() as f64) * 100.0;
        assert!(accuracy >= 95.0,
            "Accuracy below threshold: {:.2}%", accuracy);
    }
}
```

#### Preprocessing Quality Tests
```rust
#[test]
fn test_preprocessing_improves_accuracy() {
    let raw_image = load_image("test_images/noisy_character.png");

    // Test without preprocessing
    let raw_result = recognize_character_raw(raw_image.clone());

    // Test with preprocessing (grayscale, threshold, denoise)
    let preprocessed = preprocess_image(raw_image);
    let processed_result = recognize_character_raw(preprocessed);

    // Preprocessing should improve confidence
    assert!(processed_result.confidence > raw_result.confidence,
        "Preprocessing did not improve recognition confidence");
}
```

#### Edge Case Testing
```rust
#[test]
fn test_similar_character_discrimination() {
    // Test characters that look similar
    let pairs = vec![
        ("あ", "お"),  // Hiragana a vs o
        ("シ", "ツ"),  // Katakana shi vs tsu
        ("士", "土"),  // Kanji samurai vs earth
    ];

    for (char1, char2) in pairs {
        let result1 = recognize_character(&format!("test_data/{}.png", char1));
        let result2 = recognize_character(&format!("test_data/{}.png", char2));

        assert_eq!(result1.character, char1);
        assert_eq!(result2.character, char2);
        assert!(result1.confidence > 0.8 && result2.confidence > 0.8);
    }
}
```

### Metrics to Track

1. **Accuracy Rate**: Percentage of correctly recognized characters
2. **Confidence Score**: Model's certainty in predictions (0.0-1.0)
3. **Processing Time**: Latency per character recognition
4. **False Positive Rate**: Incorrect high-confidence predictions
5. **Character-Specific Performance**: Accuracy breakdown by character type (hiragana/katakana/kanji)

---

## 2. Integration Testing for Learning Flows

### Rust Integration Testing Structure

**Key Principles from Research:**
- Integration tests live in `tests/` directory adjacent to `src/`
- Each file in `tests/` is compiled as separate crate
- Tests use only public interfaces (no access to private functions)
- Test complete user flows, not individual functions
- Each test should be independent and idempotent

### Recommended Project Structure
```
japanese/
├── src/
│   ├── lib.rs
│   ├── recognition/
│   ├── learning/
│   └── gui/
├── tests/
│   ├── learning_flow_tests.rs
│   ├── recognition_integration.rs
│   └── common/
│       └── mod.rs  # Shared test utilities
└── Cargo.toml
```

### Learning Flow Test Examples

#### Complete Study Session Flow
```rust
// tests/learning_flow_tests.rs
use japanese::{LearningSession, Character, ReviewResult};

#[test]
fn test_complete_study_session_flow() {
    // 1. Initialize new learning session
    let mut session = LearningSession::new("user_123");

    // 2. User selects lesson (e.g., Hiragana basics)
    session.select_lesson("hiragana_basics");
    assert_eq!(session.pending_reviews(), 5);

    // 3. User reviews first character
    let character = session.next_character().expect("Should have character");
    assert_eq!(character.symbol, "あ");

    // 4. User attempts to write character
    let user_strokes = vec![
        Stroke::new(vec![(0, 0), (10, 10)]),
        Stroke::new(vec![(5, 5), (15, 15)]),
    ];

    // 5. System evaluates attempt
    let result = session.evaluate_attempt(&character, &user_strokes);
    assert!(result.is_correct());
    assert!(result.stroke_order_correct());

    // 6. System updates progress
    session.record_result(ReviewResult::Correct);
    assert_eq!(session.completed_reviews(), 1);
    assert_eq!(session.pending_reviews(), 4);

    // 7. Verify spaced repetition scheduling
    let next_review = session.get_next_review_time(&character);
    assert!(next_review > chrono::Utc::now());
}
```

#### Progress Persistence Flow
```rust
#[test]
fn test_progress_persistence_and_recovery() {
    // 1. Create session and make progress
    let mut session = LearningSession::new("user_456");
    session.select_lesson("katakana_basics");

    for _ in 0..3 {
        let char = session.next_character().unwrap();
        session.record_result(ReviewResult::Correct);
    }

    // 2. Save progress
    session.save().expect("Should save successfully");

    // 3. Simulate app restart
    drop(session);

    // 4. Load session from storage
    let loaded_session = LearningSession::load("user_456")
        .expect("Should load saved session");

    // 5. Verify state restored correctly
    assert_eq!(loaded_session.completed_reviews(), 3);
    assert_eq!(loaded_session.current_lesson(), "katakana_basics");
}
```

#### Error Correction Flow
```rust
#[test]
fn test_incorrect_attempt_learning_flow() {
    let mut session = LearningSession::new("user_789");
    session.select_lesson("hiragana_basics");

    let character = session.next_character().unwrap();

    // User makes incorrect attempt
    let wrong_strokes = vec![Stroke::new(vec![(0, 0), (5, 5)])];
    let result = session.evaluate_attempt(&character, &wrong_strokes);

    assert!(!result.is_correct());

    // System provides feedback
    let feedback = result.get_feedback();
    assert!(feedback.contains("stroke order"));

    // Character should be rescheduled sooner
    session.record_result(ReviewResult::Incorrect);
    let next_review = session.get_next_review_time(&character);

    // Should review again within 1 minute for failed attempts
    let expected_max = chrono::Utc::now() + chrono::Duration::minutes(1);
    assert!(next_review < expected_max);
}
```

#### Multi-Module Integration
```rust
#[test]
fn test_recognition_and_learning_integration() {
    let mut session = LearningSession::new("user_integration");

    // Load test image of handwritten character
    let handwriting_image = load_test_image("tests/fixtures/handwritten_a.png");

    // Recognition module processes it
    let recognized = session.recognize_handwriting(handwriting_image);

    // Learning module validates against current character
    let current = session.next_character().unwrap();
    let is_match = session.validate_recognition(&recognized, &current);

    assert!(is_match);
    assert!(recognized.confidence >= 0.8);
}
```

### Best Practices for Learning Flow Tests

1. **Test Independence**: Each test sets up its own state
2. **Use Test Fixtures**: Store sample data in `tests/fixtures/`
3. **Test User Journeys**: Complete paths users take through the app
4. **Mock External Dependencies**: Use traits for database/file I/O
5. **Measure Test Coverage**: Use `cargo-tarpaulin` or `cargo-llvm-cov`

---

## 3. Stroke Order Algorithm Validation

### Research-Backed Accuracy Benchmarks

**Proven Algorithm Performance:**
- Cube search: 99.17% accuracy
- Bipartite weighted matching: 99.17% accuracy
- Stable marriage algorithm: 98.54% accuracy
- Individual correspondence decision: 96.37% accuracy
- Deviation-expansion model: 96.59% accuracy

### Stroke Order Validation Approaches

#### 1. Reference Pattern Matching
```rust
#[derive(Debug, Clone)]
pub struct StrokeReference {
    strokes: Vec<Stroke>,
    character: char,
}

#[test]
fn test_stroke_order_reference_matching() {
    // Load reference pattern for character 'あ'
    let reference = StrokeReference::load('あ');
    assert_eq!(reference.strokes.len(), 3);

    // User input with correct stroke order
    let user_strokes = vec![
        Stroke::new(vec![(10, 20), (15, 25), (20, 30)]),  // Stroke 1
        Stroke::new(vec![(5, 15), (10, 20)]),             // Stroke 2
        Stroke::new(vec![(12, 18), (18, 24)]),            // Stroke 3
    ];

    let result = validate_stroke_order(&user_strokes, &reference);
    assert!(result.is_correct());
    assert_eq!(result.accuracy_score(), 1.0);
}

#[test]
fn test_incorrect_stroke_order_detection() {
    let reference = StrokeReference::load('あ');

    // User reverses stroke 2 and stroke 3
    let user_strokes = vec![
        Stroke::new(vec![(10, 20), (15, 25)]),  // Stroke 1 (correct)
        Stroke::new(vec![(12, 18), (18, 24)]),  // Stroke 3 (wrong position)
        Stroke::new(vec![(5, 15), (10, 20)]),   // Stroke 2 (wrong position)
    ];

    let result = validate_stroke_order(&user_strokes, &reference);
    assert!(!result.is_correct());
    assert!(result.accuracy_score() < 0.7);
    assert_eq!(result.first_error_position(), Some(1)); // Error at second stroke
}
```

#### 2. Shape Context Features Testing
```rust
#[test]
fn test_shape_context_matching() {
    let reference_stroke = Stroke::new(vec![(0, 0), (10, 10), (20, 5)]);
    let user_stroke = Stroke::new(vec![(1, 1), (11, 9), (19, 6)]);

    // Calculate shape context similarity
    let similarity = compute_shape_context_similarity(&user_stroke, &reference_stroke);

    // Similar strokes should have high similarity score
    assert!(similarity > 0.9, "Shape context similarity too low: {}", similarity);
}

#[test]
fn test_shape_context_rejects_dissimilar() {
    let stroke1 = Stroke::new(vec![(0, 0), (10, 10)]);     // Diagonal
    let stroke2 = Stroke::new(vec![(0, 0), (0, 10)]);      // Vertical

    let similarity = compute_shape_context_similarity(&stroke1, &stroke2);

    assert!(similarity < 0.5, "Should detect dissimilar strokes");
}
```

#### 3. Incremental Stroke Validation
```rust
#[test]
fn test_real_time_stroke_validation() {
    let mut validator = IncrementalStrokeValidator::new('あ');

    // First stroke
    let stroke1 = Stroke::new(vec![(10, 20), (15, 25)]);
    let result1 = validator.validate_next_stroke(&stroke1);
    assert!(result1.is_correct());
    assert_eq!(validator.completed_strokes(), 1);

    // Second stroke (correct)
    let stroke2 = Stroke::new(vec![(5, 15), (10, 20)]);
    let result2 = validator.validate_next_stroke(&stroke2);
    assert!(result2.is_correct());
    assert_eq!(validator.completed_strokes(), 2);

    // Attempt wrong third stroke
    let wrong_stroke = Stroke::new(vec![(0, 0), (5, 5)]);
    let result3 = validator.validate_next_stroke(&wrong_stroke);
    assert!(!result3.is_correct());
    assert!(result3.suggested_correction().is_some());
}
```

#### 4. Stroke-Number Free Recognition Testing
```rust
#[test]
fn test_stroke_number_free_recognition() {
    // Some users might combine strokes
    // Algorithm should still recognize the character

    let reference = StrokeReference::load('十');  // Character "ten" (2 strokes)

    // User draws it in one continuous stroke
    let user_strokes = vec![
        Stroke::new(vec![(5, 10), (15, 10), (10, 10), (10, 5), (10, 15)])
    ];

    let result = recognize_stroke_number_free(&user_strokes, &reference);

    // Should still recognize the character despite different stroke count
    assert_eq!(result.recognized_character, '十');
    assert!(result.confidence > 0.7);
}
```

### Validation Metrics

1. **Stroke Order Accuracy**: Percentage of strokes in correct sequence
2. **Shape Similarity Score**: 0.0-1.0 based on shape context features
3. **Temporal Consistency**: Whether stroke timing matches natural writing
4. **Direction Accuracy**: Whether stroke direction matches reference
5. **Position Tolerance**: Acceptable deviation from reference positions

### Performance Benchmarks to Achieve

- **Target Accuracy**: ≥98% for correct stroke order detection
- **Processing Latency**: <100ms per stroke validation
- **False Positive Rate**: <2% (incorrectly accepting wrong strokes)
- **False Negative Rate**: <5% (incorrectly rejecting correct strokes)

---

## 4. GUI Testing in Iced Framework

### Current State (2025)

**Challenges Identified:**
- Iced is experimental software with limited testing infrastructure
- 94.4% of Rust GUI libraries aren't production-ready
- Lacks comprehensive accessibility testing tools
- Widget customization and extension is difficult
- IME (Input Method Editor) functionality issues for complex languages

### Iced Architecture & Testing Implications

Iced follows the **Elm Architecture**:
- Separate `update()` and `view()` functions
- Unidirectional data flow
- Type-safe message passing

This architecture is actually **advantageous for testing** because:
1. Business logic (`update()`) is pure and easily testable
2. State transitions are predictable
3. View rendering is deterministic based on state

### Testing Strategies for Iced Applications

#### 1. Unit Testing the Update Logic
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_character_selection_message() {
        let mut app_state = AppState::default();

        // Simulate user clicking a character
        let message = Message::CharacterSelected('あ');
        let _cmd = app_state.update(message);

        assert_eq!(app_state.current_character, Some('あ'));
        assert_eq!(app_state.mode, Mode::Drawing);
    }

    #[test]
    fn test_stroke_completion_flow() {
        let mut app_state = AppState::new();
        app_state.current_character = Some('あ');

        // User completes drawing
        let message = Message::StrokeCompleted(vec![
            Stroke::new(vec![(0, 0), (10, 10)]),
        ]);

        let _cmd = app_state.update(message);

        assert_eq!(app_state.user_strokes.len(), 1);
        assert!(matches!(app_state.validation_state, ValidationState::Pending));
    }

    #[test]
    fn test_validation_result_handling() {
        let mut app_state = AppState::with_character('あ');

        let message = Message::ValidationComplete(ValidationResult {
            is_correct: true,
            accuracy: 0.95,
            feedback: None,
        });

        let _cmd = app_state.update(message);

        assert_eq!(app_state.validation_state, ValidationState::Success);
        assert_eq!(app_state.score, 1);
    }
}
```

#### 2. State Machine Testing
```rust
#[test]
fn test_app_state_transitions() {
    let mut app = AppState::default();

    // Initial state
    assert_eq!(app.mode, Mode::CharacterSelection);

    // Transition: Select character
    app.update(Message::CharacterSelected('あ'));
    assert_eq!(app.mode, Mode::Drawing);

    // Transition: Complete drawing
    app.update(Message::DrawingComplete);
    assert_eq!(app.mode, Mode::Validating);

    // Transition: Show results
    app.update(Message::ValidationComplete(ValidationResult::correct()));
    assert_eq!(app.mode, Mode::Results);

    // Transition: Next character
    app.update(Message::NextCharacter);
    assert_eq!(app.mode, Mode::CharacterSelection);
}
```

#### 3. Integration Testing with Mock Canvas
```rust
// Create test helper that simulates canvas interactions
pub struct MockCanvas {
    pub strokes: Vec<Stroke>,
    pub current_stroke: Vec<(f32, f32)>,
}

impl MockCanvas {
    pub fn start_stroke(&mut self, x: f32, y: f32) {
        self.current_stroke = vec![(x, y)];
    }

    pub fn add_point(&mut self, x: f32, y: f32) {
        self.current_stroke.push((x, y));
    }

    pub fn end_stroke(&mut self) {
        let stroke = Stroke::new(self.current_stroke.clone());
        self.strokes.push(stroke);
        self.current_stroke.clear();
    }
}

#[test]
fn test_drawing_interaction_with_mock_canvas() {
    let mut canvas = MockCanvas::new();
    let mut app = AppState::with_character('あ');

    // Simulate user drawing first stroke
    canvas.start_stroke(10.0, 20.0);
    canvas.add_point(15.0, 25.0);
    canvas.add_point(20.0, 30.0);
    canvas.end_stroke();

    // Send to app
    app.update(Message::StrokeAdded(canvas.strokes[0].clone()));

    assert_eq!(app.user_strokes.len(), 1);
}
```

#### 4. Snapshot Testing for View Logic
```rust
// While full visual snapshot testing is limited,
// we can test view data structures

#[test]
fn test_view_state_for_correct_answer() {
    let mut app = AppState::with_character('あ');
    app.validation_state = ValidationState::Success;
    app.score = 5;

    // Get view elements (not actual rendering)
    let view_model = app.get_view_model();

    assert_eq!(view_model.feedback_message, "Correct!");
    assert_eq!(view_model.feedback_color, Color::GREEN);
    assert_eq!(view_model.score_text, "Score: 5");
    assert!(view_model.show_next_button);
}
```

#### 5. Command Testing
```rust
#[test]
fn test_async_validation_command() {
    let mut app = AppState::with_strokes(vec![
        Stroke::new(vec![(0.0, 0.0), (10.0, 10.0)]),
    ]);

    // Update triggers validation command
    let cmd = app.update(Message::ValidateDrawing);

    // In actual implementation, this would spawn async task
    // For testing, we can verify command was created
    assert!(matches!(cmd, Command::Perform { .. }));
}
```

### Accessibility Testing Approach

Since iced lacks built-in accessibility testing:

```rust
#[test]
fn test_keyboard_navigation() {
    let mut app = AppState::default();

    // Test keyboard shortcuts
    app.update(Message::KeyPressed(KeyCode::Space));
    // Should trigger same action as clicking start
    assert_eq!(app.mode, Mode::Drawing);

    // Test escape key
    app.update(Message::KeyPressed(KeyCode::Escape));
    assert_eq!(app.mode, Mode::CharacterSelection);
}

#[test]
fn test_screen_reader_labels() {
    let app = AppState::with_character('あ');
    let view_model = app.get_view_model();

    // Ensure UI elements have descriptive labels
    assert_eq!(view_model.character_label, "Character: Hiragana A (あ)");
    assert!(!view_model.button_label.is_empty());
}
```

### Recommended Testing Tools

1. **cargo-nextest**: Faster test execution
2. **cargo-watch**: Auto-run tests on file changes
3. **insta**: Snapshot testing for data structures
4. **proptest**: Property-based testing for state machines

### GUI Testing Limitations & Workarounds

**Limitation**: Cannot easily test actual rendering
**Workaround**: Test view logic separately from rendering

**Limitation**: No built-in widget testing
**Workaround**: Extract widget logic into testable functions

**Limitation**: Async command testing is complex
**Workaround**: Use dependency injection for async operations

---

## 5. Testing Learning Effectiveness

### Key Metrics from Research

#### Kirkpatrick's Four Levels Applied to Language Learning

**Level 1: Reaction (Satisfaction)**
```rust
#[derive(Debug)]
pub struct SessionFeedback {
    pub enjoyment_rating: u8,        // 1-5
    pub difficulty_rating: u8,       // 1-5
    pub clarity_rating: u8,          // 1-5
    pub would_recommend: bool,
}

#[test]
fn test_session_feedback_collection() {
    let mut session = LearningSession::new("user_001");
    session.complete_lesson("hiragana_basics");

    let feedback = SessionFeedback {
        enjoyment_rating: 4,
        difficulty_rating: 3,
        clarity_rating: 5,
        would_recommend: true,
    };

    session.record_feedback(feedback);

    let avg_satisfaction = session.get_average_satisfaction();
    assert!(avg_satisfaction >= 4.0);
}
```

**Level 2: Learning (Knowledge Acquisition)**
```rust
#[test]
fn test_pre_post_assessment() {
    let user_id = "user_002";

    // Pre-assessment: Test baseline knowledge
    let pre_score = assess_knowledge(user_id, "hiragana_set_1");
    assert!(pre_score < 50.0); // Assumes beginner

    // User completes learning module
    let mut session = LearningSession::new(user_id);
    session.complete_lesson("hiragana_set_1");

    // Post-assessment: Test knowledge gain
    let post_score = assess_knowledge(user_id, "hiragana_set_1");

    // Should show significant improvement
    let improvement = post_score - pre_score;
    assert!(improvement >= 30.0,
        "Learning gain insufficient: {:.1}%", improvement);
}
```

**Level 3: Behavior (Application)**
```rust
#[test]
fn test_long_term_retention() {
    let user_id = "user_003";

    // Learn characters
    complete_lesson(user_id, "katakana_basics");

    // Wait period (simulated)
    simulate_time_passage(Duration::days(30));

    // Test retention without review
    let retention_score = assess_knowledge(user_id, "katakana_basics");

    // Should retain at least 70% after 30 days
    assert!(retention_score >= 70.0,
        "30-day retention too low: {:.1}%", retention_score);
}
```

**Level 4: Results (Performance Outcomes)**
```rust
#[test]
fn test_proficiency_milestone_achievement() {
    let user_id = "user_004";
    let mut session = LearningSession::new(user_id);

    // Complete curriculum
    for lesson in ["hiragana", "katakana", "kanji_n5"] {
        session.complete_lesson(lesson);
    }

    // Comprehensive proficiency test
    let proficiency = assess_overall_proficiency(user_id);

    assert!(proficiency.hiragana_mastery >= 90.0);
    assert!(proficiency.katakana_mastery >= 90.0);
    assert!(proficiency.kanji_n5_mastery >= 75.0);
    assert_eq!(proficiency.level, ProficiencyLevel::N5);
}
```

### Spaced Repetition System (SRS) Testing

#### Algorithm Validation

**Log Loss Metric (Primary SRS Benchmark)**
```rust
#[test]
fn test_srs_log_loss_metric() {
    let srs = SpacedRepetitionSystem::new();
    let test_data = load_test_reviews("test_data/review_history.json");

    let mut total_log_loss = 0.0;

    for review in test_data {
        // Predict recall probability
        let predicted_prob = srs.predict_recall_probability(&review.card);

        // Actual outcome (0 = forgot, 1 = remembered)
        let actual = if review.result == ReviewResult::Correct { 1.0 } else { 0.0 };

        // Calculate log loss for this prediction
        let log_loss = -(actual * predicted_prob.ln() +
                        (1.0 - actual) * (1.0 - predicted_prob).ln());
        total_log_loss += log_loss;
    }

    let avg_log_loss = total_log_loss / test_data.len() as f64;

    // Lower is better; research shows good algorithms achieve 0.3-0.4
    assert!(avg_log_loss < 0.5,
        "Log loss too high: {:.3}", avg_log_loss);
}
```

#### Interval Scheduling Validation
```rust
#[test]
fn test_srs_interval_expansion() {
    let mut srs = SpacedRepetitionSystem::new();
    let card = Card::new("あ");

    // First review (correct)
    let interval1 = srs.schedule_next_review(&card, ReviewResult::Correct);
    assert!(interval1 >= Duration::minutes(10));
    assert!(interval1 <= Duration::hours(1));

    // Second review (correct)
    srs.record_review(&card, ReviewResult::Correct);
    let interval2 = srs.schedule_next_review(&card, ReviewResult::Correct);
    assert!(interval2 > interval1);
    assert!(interval2 >= Duration::hours(1));

    // Third review (correct)
    srs.record_review(&card, ReviewResult::Correct);
    let interval3 = srs.schedule_next_review(&card, ReviewResult::Correct);
    assert!(interval3 > interval2);
    assert!(interval3 >= Duration::days(1));

    // Intervals should expand with successful reviews
    assert!(interval1 < interval2 && interval2 < interval3);
}

#[test]
fn test_srs_interval_contraction_on_failure() {
    let mut srs = SpacedRepetitionSystem::new();
    let card = Card::new("お");

    // Build up some interval
    for _ in 0..3 {
        srs.record_review(&card, ReviewResult::Correct);
    }

    let long_interval = srs.schedule_next_review(&card, ReviewResult::Correct);
    assert!(long_interval >= Duration::days(1));

    // User forgets
    srs.record_review(&card, ReviewResult::Incorrect);
    let reset_interval = srs.schedule_next_review(&card, ReviewResult::Incorrect);

    // Should dramatically reduce interval
    assert!(reset_interval < Duration::hours(1));
    assert!(reset_interval < long_interval);
}
```

#### Forgetting Curve Validation
```rust
#[test]
fn test_forgetting_curve_modeling() {
    let srs = SpacedRepetitionSystem::new();
    let card = Card::with_history(vec![
        Review { result: ReviewResult::Correct, timestamp: days_ago(10) },
        Review { result: ReviewResult::Correct, timestamp: days_ago(7) },
        Review { result: ReviewResult::Correct, timestamp: days_ago(3) },
    ]);

    // Predict recall probability at different time points
    let prob_now = srs.predict_recall_probability_at(&card, now());
    let prob_1day = srs.predict_recall_probability_at(&card, days_from_now(1));
    let prob_7days = srs.predict_recall_probability_at(&card, days_from_now(7));

    // Probability should decrease over time (forgetting curve)
    assert!(prob_now > prob_1day);
    assert!(prob_1day > prob_7days);

    // Should still have reasonable retention for recently reviewed item
    assert!(prob_now >= 0.9);
}
```

### Completion Rate Testing

```rust
#[test]
fn test_lesson_completion_rate() {
    let cohort = create_test_cohort(100); // 100 test users

    for user in &cohort {
        user.start_lesson("hiragana_basics");
    }

    // Simulate learning sessions
    simulate_learning_period(Duration::weeks(2));

    let completed = cohort.iter()
        .filter(|u| u.has_completed_lesson("hiragana_basics"))
        .count();

    let completion_rate = (completed as f64 / cohort.len() as f64) * 100.0;

    // Target: >70% completion rate indicates engaging content
    assert!(completion_rate >= 70.0,
        "Completion rate too low: {:.1}%", completion_rate);
}
```

### Assessment Score Analysis

```rust
#[test]
fn test_assessment_score_distribution() {
    let test_results = run_assessment_cohort(50, "kanji_n5_final");

    let mean_score = test_results.mean();
    let std_dev = test_results.std_dev();

    // Good learning system: mean ≥75%, reasonable standard deviation
    assert!(mean_score >= 75.0, "Mean score too low: {:.1}", mean_score);
    assert!(std_dev >= 10.0 && std_dev <= 20.0,
        "Score distribution unhealthy: σ={:.1}", std_dev);

    // Check for ceiling effects (too many perfect scores = test too easy)
    let perfect_scores = test_results.iter().filter(|&s| *s == 100.0).count();
    let ceiling_rate = (perfect_scores as f64 / test_results.len() as f64) * 100.0;
    assert!(ceiling_rate < 30.0, "Test may be too easy");

    // Check for floor effects (too many failures = test too hard)
    let failing_scores = test_results.iter().filter(|&s| *s < 60.0).count();
    let floor_rate = (failing_scores as f64 / test_results.len() as f64) * 100.0;
    assert!(floor_rate < 20.0, "Test may be too difficult");
}
```

### A/B Testing for Learning Methods

```rust
#[test]
fn test_learning_method_effectiveness_comparison() {
    // Group A: Traditional flashcards
    let group_a = create_test_cohort(50);
    for user in &group_a {
        user.use_learning_method(LearningMethod::TraditionalFlashcards);
        user.complete_lesson("kanji_basics");
    }

    // Group B: Stroke order practice with immediate feedback
    let group_b = create_test_cohort(50);
    for user in &group_b {
        user.use_learning_method(LearningMethod::InteractiveStrokeOrder);
        user.complete_lesson("kanji_basics");
    }

    // Compare retention after 1 week
    simulate_time_passage(Duration::weeks(1));

    let score_a = assess_cohort_knowledge(&group_a, "kanji_basics");
    let score_b = assess_cohort_knowledge(&group_b, "kanji_basics");

    // Expected: Interactive method should show better retention
    let improvement = score_b - score_a;
    assert!(improvement >= 10.0,
        "Interactive method should show ≥10% improvement, got {:.1}%", improvement);
}
```

### Time-to-Proficiency Metrics

```rust
#[test]
fn test_time_to_proficiency_benchmark() {
    let mut session = LearningSession::new("benchmark_user");

    let start_time = Instant::now();

    // User learns until reaching proficiency threshold
    while session.proficiency_score("hiragana") < 90.0 {
        session.study_session(Duration::minutes(30));
    }

    let time_to_proficiency = start_time.elapsed();

    // Benchmark: Should achieve 90% proficiency within 10 hours
    assert!(time_to_proficiency <= Duration::hours(10),
        "Time to proficiency exceeds benchmark: {:?}", time_to_proficiency);
}
```

---

## 6. Comprehensive Testing Strategy

### Testing Pyramid for Educational Rust App

```
                  ╱╲
                 ╱  ╲
                ╱ E2E ╲         <- Few: Complete user journeys
               ╱────────╲
              ╱          ╲
             ╱Integration ╲     <- Some: Module interactions, learning flows
            ╱──────────────╲
           ╱                ╲
          ╱   Unit Tests     ╲  <- Many: Recognition, SRS, validation logic
         ╱────────────────────╲
```

### Recommended Test Coverage Targets

- **Unit Tests**: ≥80% code coverage
- **Integration Tests**: All critical user flows
- **Learning Effectiveness**: Ongoing cohort analysis
- **Performance Tests**: Benchmarks for recognition/validation latency

### CI/CD Integration

```yaml
# .github/workflows/test.yml
name: Test Suite

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rust-lang/setup-rust-toolchain@v1

      - name: Run unit tests
        run: cargo test --lib

      - name: Run integration tests
        run: cargo test --test '*'

      - name: Run benchmarks
        run: cargo bench --no-run

      - name: Check code coverage
        run: |
          cargo install cargo-tarpaulin
          cargo tarpaulin --out Xml --output-dir coverage

      - name: Upload coverage
        uses: codecov/codecov-action@v3
```

### Performance Benchmarking

```rust
// benches/recognition_bench.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn benchmark_character_recognition(c: &mut Criterion) {
    let test_image = load_test_image("benches/fixtures/sample_character.png");

    c.bench_function("recognize_character", |b| {
        b.iter(|| {
            recognize_character(black_box(&test_image))
        });
    });
}

fn benchmark_stroke_validation(c: &mut Criterion) {
    let reference = StrokeReference::load('あ');
    let user_strokes = load_test_strokes("benches/fixtures/user_strokes.json");

    c.bench_function("validate_stroke_order", |b| {
        b.iter(|| {
            validate_stroke_order(black_box(&user_strokes), black_box(&reference))
        });
    });
}

criterion_group!(benches, benchmark_character_recognition, benchmark_stroke_validation);
criterion_main!(benches);
```

---

## 7. Tools and Dependencies

### Recommended Cargo.toml additions

```toml
[dev-dependencies]
# Testing frameworks
criterion = "0.5"              # Benchmarking
proptest = "1.4"               # Property-based testing
insta = "1.34"                 # Snapshot testing
mockall = "0.12"               # Mocking

# Test data
serde_json = "1.0"             # Test fixtures
tempfile = "3.8"               # Temporary test files

# Coverage
tarpaulin = "0.27"             # Code coverage

# Integration testing helpers
assert_cmd = "2.0"             # CLI testing
predicates = "3.0"             # Assertion helpers

# Educational metrics
statistical = "1.0"            # Statistical analysis for learning metrics
```

---

## 8. Key Takeaways

### Character Recognition Testing
- Target ≥95% accuracy on test datasets
- Test preprocessing pipeline improvements
- Validate confidence scores
- Test edge cases (similar characters)

### Learning Flow Testing
- Use integration tests in `tests/` directory
- Test complete user journeys
- Ensure test independence
- Mock external dependencies

### Stroke Order Validation
- Aim for ≥98% accuracy (research-backed)
- Use shape context features
- Implement incremental validation
- Test stroke-number-free recognition

### GUI Testing in Iced
- Focus on testing `update()` logic (pure functions)
- Test state transitions
- Mock canvas interactions
- Use snapshot testing for view models
- Work around rendering test limitations

### Learning Effectiveness
- Implement Kirkpatrick's four levels
- Use Log Loss for SRS validation (target <0.5)
- Track completion rates (target ≥70%)
- Monitor time-to-proficiency benchmarks
- A/B test learning methods
- Validate forgetting curve modeling

### Overall Strategy
- Follow testing pyramid (many unit, some integration, few E2E)
- Maintain ≥80% code coverage
- Integrate benchmarks into CI/CD
- Continuously measure learning outcomes
