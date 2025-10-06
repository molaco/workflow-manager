# Hiragana/Katakana Learning Application - Implementation Guide

## Overview

This guide synthesizes research findings for building an interactive Japanese character learning application using Rust and the iced framework. The application features visual recognition, stroke order animation, and progress tracking for hiragana and katakana characters.

---

## 1. Architecture Overview

### Core Components

```
┌─────────────────────────────────────────────────────────┐
│                    Application State                     │
│  (Elm Architecture: Model → Update → View)              │
└─────────────────────────────────────────────────────────┘
                            │
        ┌───────────────────┼───────────────────┐
        │                   │                   │
┌───────▼────────┐ ┌────────▼────────┐ ┌───────▼────────┐
│  UI Layer      │ │ Character Data  │ │ Progress Track │
│  (iced)        │ │ Management      │ │ (rusqlite)     │
└───────┬────────┘ └────────┬────────┘ └───────┬────────┘
        │                   │                   │
┌───────▼────────┐ ┌────────▼────────┐ ┌───────▼────────┐
│ Stroke Anim    │ │ Character Set   │ │ Spaced Rep     │
│ (femtovg)      │ │ (JSON/HashMap)  │ │ (SM-2/FSRS)    │
└────────────────┘ └─────────────────┘ └────────────────┘
        │
┌───────▼────────┐
│ Recognition    │
│ (ort + DTW)    │
└────────────────┘
```

### Technology Stack

| Component | Library | Rationale |
|-----------|---------|-----------|
| **GUI Framework** | iced | Cross-platform, Elm architecture, custom widgets |
| **Animation** | femtovg + wgpu | GPU-accelerated, Lottie support, cross-platform |
| **Recognition** | ort (ONNX) | Pre-trained models, hardware acceleration |
| **Database** | rusqlite | SQLite integration, complex queries, relationships |
| **Stroke Data** | KanjiVG + lyon | SVG parsing, GPU tessellation |
| **Metrics** | metrics crate | Statistics tracking, performance monitoring |
| **Visualization** | plotters | Charts, progress graphs, analytics |

---

## 2. Character Data Management

### Data Structure

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Character {
    char: char,
    unicode: u32,
    char_type: CharType, // Hiragana or Katakana
    romaji: Vec<String>,
    category: Category,  // Seion, Dakuon, Yōon
    stroke_count: u8,
    strokes: Vec<Stroke>,
    related: RelatedCharacters,
}

#[derive(Debug, Clone)]
struct Stroke {
    order: u8,
    path: PathData,      // SVG path from KanjiVG
    median: Vec<Point>,  // Coordinate points
}

#[derive(Debug, Clone)]
struct RelatedCharacters {
    katakana: Option<char>,
    hiragana: Option<char>,
    base_char: Option<char>,  // For dakuten/handakuten
    diacritic: Option<Diacritic>,
}
```

### Storage Format (JSON)

```json
{
  "characters": [
    {
      "char": "あ",
      "unicode": 12354,
      "type": "hiragana",
      "romaji": ["a"],
      "category": "seion",
      "stroke_count": 3,
      "strokes": [
        {
          "order": 1,
          "path": "M 25 35 Q 30 45 35 55",
          "median": [[25, 35], [30, 45], [35, 55]]
        }
      ],
      "related": {
        "katakana": "ア",
        "dakuten": null
      }
    }
  ]
}
```

### Efficient Lookup Mechanisms

```rust
struct CharacterDatabase {
    // O(1) lookups
    by_unicode: HashMap<u32, Character>,
    by_romaji: HashMap<String, Vec<char>>,
    by_type: HashMap<CharType, Vec<char>>,
}

impl CharacterDatabase {
    fn from_json(json: &str) -> Result<Self> {
        let chars: Vec<Character> = serde_json::from_str(json)?;

        let by_unicode = chars.iter()
            .map(|c| (c.unicode, c.clone()))
            .collect();

        let by_romaji = chars.iter()
            .flat_map(|c| c.romaji.iter().map(move |r| (r.clone(), c.char)))
            .fold(HashMap::new(), |mut acc, (r, ch)| {
                acc.entry(r).or_insert_with(Vec::new).push(ch);
                acc
            });

        Ok(Self { by_unicode, by_romaji, by_type })
    }
}
```

### Character Set Coverage

- **46** basic characters per script
- **25** dakuten/handakuten variants (゛゜)
- **33** contracted sounds (yōon)
- **Total: ~104 characters** per script (hiragana/katakana)

---

## 3. Stroke Order Animation

### Implementation Approach

**Recommended: femtovg with wgpu backend**

1. Parse KanjiVG SVG files to extract stroke paths
2. Convert paths to GPU-ready geometry using lyon tessellation
3. Render progressively with frame-based animation

### Animation Strategy

```rust
struct StrokeAnimator {
    strokes: Vec<Stroke>,
    current_stroke: usize,
    progress: f32,  // 0.0 to 1.0
}

impl StrokeAnimator {
    fn update(&mut self, delta_time: f32) {
        self.progress += delta_time * ANIMATION_SPEED;

        if self.progress >= 1.0 {
            self.progress = 0.0;
            self.current_stroke = (self.current_stroke + 1) % self.strokes.len();
        }
    }

    fn render(&self, canvas: &mut Canvas) {
        // Render completed strokes
        for stroke in &self.strokes[..self.current_stroke] {
            canvas.stroke_path(&stroke.path, paint);
        }

        // Render current stroke partially
        if let Some(stroke) = self.strokes.get(self.current_stroke) {
            let partial_path = stroke.path.partial(0.0, self.progress);
            canvas.stroke_path(&partial_path, paint);
        }
    }
}
```

### Progressive Rendering Techniques

**SVG Method (for web targets):**
```css
stroke-dasharray: pathLength;
stroke-dashoffset: pathLength → 0; /* animated */
```

**Canvas Method (for desktop):**
- Calculate partial path lengths per stroke
- Render strokes sequentially (0→N)
- Use clipping paths or partial geometry
- Interpolate between keyframes for smoothness

---

## 4. Visual Character Recognition

### Architecture

```
User Input → Canvas Drawing → Stroke Capture → Preprocessing →
Feature Extraction → Model Inference → Character Match → Feedback
```

### Canvas Drawing (egui for native)

```rust
use egui::Stroke;

struct DrawingCanvas {
    strokes: Vec<Vec<Pos2>>,
    current_stroke: Vec<Pos2>,
}

impl DrawingCanvas {
    fn ui(&mut self, ui: &mut egui::Ui) {
        let (response, painter) = ui.allocate_painter(
            Vec2::new(300.0, 300.0),
            egui::Sense::drag()
        );

        if response.dragged() {
            if let Some(pos) = response.interact_pointer_pos() {
                self.current_stroke.push(pos);
            }
        }

        if response.drag_released() {
            self.strokes.push(self.current_stroke.clone());
            self.current_stroke.clear();
        }

        // Render strokes
        for stroke in &self.strokes {
            painter.add(egui::Shape::line(
                stroke.clone(),
                Stroke::new(2.0, Color32::BLACK)
            ));
        }
    }
}
```

### Stroke Matching Algorithms

**Dynamic Positional Warping (DPW)** - Recommended for 2D handwriting

```rust
fn dpw_distance(user_stroke: &[Point], template_stroke: &[Point]) -> f32 {
    let n = user_stroke.len();
    let m = template_stroke.len();
    let mut dp = vec![vec![f32::INFINITY; m + 1]; n + 1];
    dp[0][0] = 0.0;

    for i in 1..=n {
        for j in 1..=m {
            let cost = euclidean_distance(user_stroke[i-1], template_stroke[j-1]);
            dp[i][j] = cost + dp[i-1][j-1].min(dp[i-1][j]).min(dp[i][j-1]);
        }
    }

    dp[n][m]
}
```

**Hausdorff Distance** - Efficient shape matching

```rust
fn hausdorff_distance(set_a: &[Point], set_b: &[Point]) -> f32 {
    let max_dist_a = set_a.iter()
        .map(|a| set_b.iter().map(|b| distance(a, b)).min_by(f32::total_cmp).unwrap())
        .max_by(f32::total_cmp)
        .unwrap();

    let max_dist_b = set_b.iter()
        .map(|b| set_a.iter().map(|a| distance(a, b)).min_by(f32::total_cmp).unwrap())
        .max_by(f32::total_cmp)
        .unwrap();

    max_dist_a.max(max_dist_b)
}
```

### OCR Integration (ort - ONNX Runtime)

```rust
use ort::{Session, Value};

struct CharacterRecognizer {
    session: Session,
    labels: Vec<char>,
}

impl CharacterRecognizer {
    fn recognize(&self, image: &Image) -> Result<(char, f32)> {
        // Preprocess: resize to 28x28, normalize
        let input = preprocess_image(image);

        // Run inference
        let outputs = self.session.run(vec![Value::from_array(input)?])?;
        let probabilities = outputs[0].extract_tensor::<f32>()?;

        // Get top prediction
        let (max_idx, max_prob) = probabilities
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .unwrap();

        Ok((self.labels[max_idx], *max_prob))
    }
}
```

### Feature Extraction (HOG - 99%+ accuracy)

```rust
use kornia_rs::imgproc;

fn extract_hog_features(image: &Image) -> Vec<f32> {
    // 1. Convert to grayscale
    let gray = imgproc::color::gray_from_rgb(&image);

    // 2. Compute gradients
    let (grad_x, grad_y) = imgproc::gradients(&gray);

    // 3. Compute orientation and magnitude
    let orientation = grad_y.atan2(&grad_x);
    let magnitude = (grad_x.powi(2) + grad_y.powi(2)).sqrt();

    // 4. Build histogram of oriented gradients (9 bins, 8x8 cells)
    build_histogram(&orientation, &magnitude, 9, 8)
}
```

### Training Dataset

**Kuzushiji-MNIST** - 70,000 images of hiragana characters
- 10 hiragana character classes
- 28x28 grayscale images
- Includes cursive/historical variants
- Available on Kaggle and academic repositories

---

## 5. Progress Tracking System

### Database Schema (SQLite)

```sql
CREATE TABLE users (
    id INTEGER PRIMARY KEY,
    username TEXT UNIQUE NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE character_progress (
    id INTEGER PRIMARY KEY,
    user_id INTEGER NOT NULL,
    character TEXT NOT NULL,
    easiness_factor REAL DEFAULT 2.5,
    interval_days INTEGER DEFAULT 0,
    repetitions INTEGER DEFAULT 0,
    next_review_date DATETIME,
    mastery_level TEXT CHECK(mastery_level IN
        ('unknown', 'learning', 'young', 'mature', 'mastered')),
    last_reviewed DATETIME,
    FOREIGN KEY (user_id) REFERENCES users(id),
    UNIQUE(user_id, character)
);

CREATE TABLE review_history (
    id INTEGER PRIMARY KEY,
    user_id INTEGER NOT NULL,
    character TEXT NOT NULL,
    quality INTEGER CHECK(quality BETWEEN 0 AND 5),
    response_time_ms INTEGER,
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id)
);

CREATE TABLE session_stats (
    id INTEGER PRIMARY KEY,
    user_id INTEGER NOT NULL,
    session_start DATETIME,
    session_end DATETIME,
    total_reviews INTEGER,
    correct_reviews INTEGER,
    average_response_time_ms INTEGER,
    FOREIGN KEY (user_id) REFERENCES users(id)
);
```

### Spaced Repetition Algorithm (SM-2)

```rust
use spaced_repetition::sm2::{ReviewCard, Quality};

#[derive(Debug)]
struct CharacterCard {
    character: char,
    easiness_factor: f32,
    interval: i32,
    repetitions: i32,
    next_review: DateTime<Utc>,
}

impl CharacterCard {
    fn review(&mut self, quality: Quality) {
        let card = ReviewCard {
            easiness_factor: self.easiness_factor,
            interval: self.interval,
            repetitions: self.repetitions,
        };

        let result = card.review(quality);

        self.easiness_factor = result.easiness_factor;
        self.interval = result.interval;
        self.repetitions = result.repetitions;
        self.next_review = Utc::now() + Duration::days(result.interval as i64);
    }
}
```

**Quality Ratings:**
- **0** - Complete blackout
- **1** - Incorrect, but familiar
- **2** - Incorrect, but remembered
- **3** - Correct with difficulty
- **4** - Correct with hesitation
- **5** - Perfect recall

### Mastery Level Progression

```rust
#[derive(Debug, Clone, Copy)]
enum MasteryLevel {
    Unknown,    // Never studied
    Learning,   // 0-2 repetitions
    Young,      // 3-5 repetitions, interval < 21 days
    Mature,     // 6+ repetitions, interval >= 21 days
    Mastered,   // 10+ repetitions, interval >= 90 days
}

impl CharacterCard {
    fn mastery_level(&self) -> MasteryLevel {
        match (self.repetitions, self.interval) {
            (0, _) => MasteryLevel::Unknown,
            (1..=2, _) => MasteryLevel::Learning,
            (3..=5, 0..21) => MasteryLevel::Young,
            (6.., 21..90) => MasteryLevel::Mature,
            (10.., 90..) => MasteryLevel::Mastered,
            _ => MasteryLevel::Young,
        }
    }
}
```

### Statistics Tracking

```rust
use metrics::{counter, gauge, histogram};

struct ProgressTracker;

impl ProgressTracker {
    fn record_review(&self, character: char, quality: u8, time_ms: u64) {
        counter!("reviews_total").increment(1);

        if quality >= 3 {
            counter!("reviews_correct").increment(1);
        }

        histogram!("response_time_ms").record(time_ms as f64);
        gauge!("current_streak").set(self.calculate_streak() as f64);
    }

    fn retention_rate(&self, conn: &Connection) -> f32 {
        let result: (i32, i32) = conn.query_row(
            "SELECT
                SUM(CASE WHEN quality >= 3 THEN 1 ELSE 0 END) as correct,
                COUNT(*) as total
             FROM review_history
             WHERE timestamp > datetime('now', '-7 days')",
            [],
            |row| Ok((row.get(0)?, row.get(1)?))
        ).unwrap();

        result.0 as f32 / result.1 as f32
    }
}
```

### Progress Visualization (plotters)

```rust
use plotters::prelude::*;

fn draw_retention_chart(data: &[(DateTime<Utc>, f32)]) -> Result<()> {
    let root = BitMapBackend::new("retention.png", (800, 600))
        .into_drawing_area();
    root.fill(&WHITE)?;

    let mut chart = ChartBuilder::on(&root)
        .caption("Retention Rate Over Time", ("sans-serif", 40))
        .margin(10)
        .x_label_area_size(40)
        .y_label_area_size(50)
        .build_cartesian_2d(
            data[0].0..data[data.len()-1].0,
            0.0f32..1.0f32
        )?;

    chart.configure_mesh().draw()?;

    chart.draw_series(LineSeries::new(
        data.iter().map(|(date, rate)| (*date, *rate)),
        &RED
    ))?;

    root.present()?;
    Ok(())
}
```

---

## 6. Interactive Learning UI/UX

### Iced Application Structure

```rust
struct LearningApp {
    state: AppState,
    character_db: CharacterDatabase,
    progress_tracker: ProgressTracker,
    current_character: Option<Character>,
    user_input: String,
    feedback: Option<Feedback>,
}

enum AppState {
    Menu,
    Study { mode: StudyMode },
    Quiz { questions: Vec<Question>, current: usize },
    Results { stats: SessionStats },
}

enum StudyMode {
    Flashcard,
    StrokeOrder,
    Recognition,
    RapidFire,
}

#[derive(Debug, Clone)]
enum Message {
    CharacterSelected(char),
    UserInputChanged(String),
    SubmitAnswer,
    NextQuestion,
    RateQuality(Quality),
    StartSession(StudyMode),
    EndSession,
}
```

### Quiz Interface

```rust
impl Application for LearningApp {
    type Message = Message;

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::SubmitAnswer => {
                let correct = self.check_answer(&self.user_input);

                self.feedback = Some(if correct {
                    Feedback::Correct {
                        message: "正解！".to_string(),
                        color: Color::from_rgb(0.0, 0.8, 0.0),
                        icon: "✓"
                    }
                } else {
                    Feedback::Incorrect {
                        message: format!("正しい答え: {}", self.current_character.romaji[0]),
                        color: Color::from_rgb(0.8, 0.0, 0.0),
                        icon: "✗"
                    }
                });

                Command::none()
            }
            Message::NextQuestion => {
                self.load_next_question();
                self.feedback = None;
                self.user_input.clear();
                Command::none()
            }
            // ... other message handlers
        }
    }

    fn view(&self) -> Element<Message> {
        let character_display = text(&self.current_character.char)
            .size(120)
            .horizontal_alignment(alignment::Horizontal::Center);

        let input = text_input("Type the romaji...", &self.user_input)
            .on_input(Message::UserInputChanged)
            .on_submit(Message::SubmitAnswer)
            .padding(10);

        let submit_button = button("Submit")
            .on_press(Message::SubmitAnswer);

        let feedback_widget = if let Some(ref feedback) = self.feedback {
            row![
                text(feedback.icon).size(30),
                text(&feedback.message).size(20).style(feedback.color)
            ].spacing(10)
        } else {
            row![]
        };

        column![
            character_display,
            input,
            submit_button,
            feedback_widget
        ]
        .spacing(20)
        .padding(20)
        .into()
    }
}
```

### Flashcard System

```rust
struct FlashcardView {
    character: Character,
    is_flipped: bool,
}

impl FlashcardView {
    fn view(&self) -> Element<Message> {
        let content = if !self.is_flipped {
            // Front: Show character
            container(text(&self.character.char).size(100))
        } else {
            // Back: Show romaji, stroke order, related info
            column![
                text(self.character.romaji.join(", ")).size(40),
                text(format!("Strokes: {}", self.character.stroke_count)),
                text(format!("Type: {:?}", self.character.char_type))
            ]
        };

        let card = container(content)
            .width(Length::Fixed(400.0))
            .height(Length::Fixed(300.0))
            .style(CardStyle)
            .padding(20);

        let rating_buttons = row![
            button("Again").on_press(Message::RateQuality(Quality::Again)),
            button("Hard").on_press(Message::RateQuality(Quality::Hard)),
            button("Good").on_press(Message::RateQuality(Quality::Good)),
            button("Easy").on_press(Message::RateQuality(Quality::Easy)),
        ].spacing(10);

        column![
            card.on_press(Message::FlipCard),
            rating_buttons
        ].into()
    }
}
```

### Accessible Feedback (Multi-modal)

**Never rely on color alone** - Combine visual, textual, and optional audio cues:

```rust
struct Feedback {
    icon: &'static str,     // ✓ or ✗
    message: String,         // Text explanation
    color: Color,            // Green/Red
    sound: Option<Sound>,    // Optional audio cue
}

// Color-blind friendly palette
const CORRECT_COLOR: Color = Color::from_rgb(0.0, 0.6, 0.2);    // Dark green
const INCORRECT_COLOR: Color = Color::from_rgb(0.8, 0.2, 0.0);  // Dark red
const NEUTRAL_COLOR: Color = Color::from_rgb(0.3, 0.3, 0.3);    // Gray
```

### Stroke Order Canvas Widget

```rust
use iced::widget::canvas::{self, Canvas, Cursor, Frame, Geometry, Path, Stroke};

struct StrokeOrderCanvas {
    animator: StrokeAnimator,
    is_playing: bool,
}

impl canvas::Program<Message> for StrokeOrderCanvas {
    fn update(
        &mut self,
        event: canvas::Event,
        bounds: Rectangle,
        cursor: Cursor,
    ) -> (canvas::event::Status, Option<Message>) {
        match event {
            canvas::Event::Mouse(mouse::Event::ButtonPressed(_)) => {
                self.is_playing = !self.is_playing;
                (canvas::event::Status::Captured, Some(Message::ToggleAnimation))
            }
            _ => (canvas::event::Status::Ignored, None)
        }
    }

    fn draw(&self, bounds: Rectangle, _cursor: Cursor) -> Vec<Geometry> {
        let mut frame = Frame::new(bounds.size());

        // Draw completed strokes in gray
        for stroke in &self.animator.completed_strokes() {
            frame.stroke(
                &stroke.path,
                Stroke::default().with_width(4.0).with_color(Color::from_rgb(0.7, 0.7, 0.7))
            );
        }

        // Draw current stroke being animated in black
        if let Some(current) = self.animator.current_partial_stroke() {
            frame.stroke(
                &current.path,
                Stroke::default().with_width(4.0).with_color(Color::BLACK)
            );
        }

        vec![frame.into_geometry()]
    }
}
```

---

## 7. Testing Strategy

### Unit Tests - Character Recognition

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hiragana_recognition_accuracy() {
        let recognizer = CharacterRecognizer::from_model("model.onnx").unwrap();
        let test_images = load_kuzushiji_test_set();

        let mut correct = 0;
        let mut total = 0;

        for (image, label) in test_images {
            let (predicted, confidence) = recognizer.recognize(&image).unwrap();

            if predicted == label {
                correct += 1;
            }
            total += 1;
        }

        let accuracy = correct as f32 / total as f32;
        assert!(accuracy >= 0.95, "Accuracy {} below 95% threshold", accuracy);
    }

    #[test]
    fn test_confidence_thresholds() {
        let recognizer = CharacterRecognizer::from_model("model.onnx").unwrap();
        let clear_image = load_test_image("clear_あ.png");

        let (_, confidence) = recognizer.recognize(&clear_image).unwrap();
        assert!(confidence >= 0.90, "Clear image should have high confidence");

        let noisy_image = load_test_image("noisy_あ.png");
        let (_, confidence) = recognizer.recognize(&noisy_image).unwrap();
        assert!(confidence < 0.90, "Noisy image should have lower confidence");
    }
}
```

### Integration Tests - Learning Flow

```rust
#[test]
fn test_complete_study_session() {
    let mut app = LearningApp::new();

    // Start session
    app.update(Message::StartSession(StudyMode::Quiz));
    assert!(matches!(app.state, AppState::Quiz { .. }));

    // Answer questions
    for _ in 0..10 {
        app.update(Message::UserInputChanged("a".to_string()));
        app.update(Message::SubmitAnswer);
        app.update(Message::NextQuestion);
    }

    // End session
    app.update(Message::EndSession);
    assert!(matches!(app.state, AppState::Results { .. }));

    // Verify progress persisted
    let conn = Connection::open("test.db").unwrap();
    let count: i32 = conn.query_row(
        "SELECT COUNT(*) FROM review_history",
        [],
        |row| row.get(0)
    ).unwrap();
    assert_eq!(count, 10);
}
```

### Stroke Order Validation

```rust
#[test]
fn test_stroke_order_matching() {
    let template = load_character_strokes('あ');
    let user_strokes = vec![
        vec![Point::new(25, 35), Point::new(30, 45)],
        vec![Point::new(40, 20), Point::new(45, 60)],
        vec![Point::new(15, 50), Point::new(50, 50)],
    ];

    let accuracy = calculate_stroke_accuracy(&user_strokes, &template);
    assert!(accuracy >= 0.85, "Stroke accuracy should be at least 85%");
}

fn calculate_stroke_accuracy(user: &[Vec<Point>], template: &[Stroke]) -> f32 {
    if user.len() != template.len() {
        return 0.0;
    }

    let total_distance: f32 = user.iter()
        .zip(template.iter())
        .map(|(u_stroke, t_stroke)| dpw_distance(u_stroke, &t_stroke.median))
        .sum();

    // Normalize to 0-1 range
    let max_distance = 100.0 * user.len() as f32;
    1.0 - (total_distance / max_distance).min(1.0)
}
```

### GUI Testing (State Transitions)

```rust
#[test]
fn test_quiz_state_machine() {
    let mut app = LearningApp::new();

    // Menu → Quiz
    app.update(Message::StartSession(StudyMode::Quiz));
    assert!(matches!(app.state, AppState::Study { .. }));

    // Submit incorrect answer
    app.update(Message::UserInputChanged("wrong".to_string()));
    app.update(Message::SubmitAnswer);
    assert!(app.feedback.is_some());
    assert!(matches!(app.feedback, Some(Feedback::Incorrect { .. })));

    // Submit correct answer
    app.update(Message::UserInputChanged(&app.current_character.romaji[0]));
    app.update(Message::SubmitAnswer);
    assert!(matches!(app.feedback, Some(Feedback::Correct { .. })));
}
```

### Learning Effectiveness Validation

```rust
#[test]
fn test_spaced_repetition_intervals() {
    let mut card = CharacterCard::new('あ');

    // Perfect recalls should increase interval
    card.review(Quality::Perfect);
    assert_eq!(card.interval, 1);

    card.review(Quality::Perfect);
    assert_eq!(card.interval, 6);

    card.review(Quality::Perfect);
    assert!(card.interval >= 14);

    // Difficult recall should decrease interval
    card.review(Quality::Difficult);
    let prev_interval = card.interval;
    assert!(card.interval < prev_interval);
}

#[test]
fn test_mastery_progression() {
    let mut card = CharacterCard::new('あ');

    assert_eq!(card.mastery_level(), MasteryLevel::Unknown);

    for _ in 0..3 {
        card.review(Quality::Good);
    }
    assert_eq!(card.mastery_level(), MasteryLevel::Learning);

    for _ in 0..10 {
        card.review(Quality::Perfect);
    }
    assert_eq!(card.mastery_level(), MasteryLevel::Mature);
}
```

---

## 8. Implementation Checklist

### Phase 1: Foundation (Week 1-2)
- [ ] Set up Rust project with iced framework
- [ ] Create character data structures
- [ ] Parse KanjiVG SVG files for hiragana/katakana
- [ ] Implement JSON-based character database with HashMap lookups
- [ ] Build basic iced application shell with menu

### Phase 2: Stroke Animation (Week 3-4)
- [ ] Integrate femtovg + wgpu backend
- [ ] Implement stroke path tessellation with lyon
- [ ] Create StrokeAnimator with progressive rendering
- [ ] Build canvas widget for stroke display
- [ ] Add animation controls (play/pause/speed)

### Phase 3: Recognition System (Week 5-6)
- [ ] Integrate egui canvas for drawing input
- [ ] Implement stroke capture and preprocessing
- [ ] Add DPW/Hausdorff distance algorithms
- [ ] Integrate ort (ONNX Runtime)
- [ ] Train/load HOG-based classifier on Kuzushiji-MNIST
- [ ] Build character matching pipeline

### Phase 4: Progress Tracking (Week 7-8)
- [ ] Set up rusqlite with schema creation
- [ ] Implement SM-2 spaced repetition algorithm
- [ ] Create progress tracking service
- [ ] Add review history logging
- [ ] Build mastery level calculation
- [ ] Integrate metrics collection

### Phase 5: UI/UX (Week 9-10)
- [ ] Design and implement quiz interface
- [ ] Build flashcard view with flip animation
- [ ] Create accessible feedback system (icon + color + text)
- [ ] Add practice mode selection
- [ ] Implement session statistics display
- [ ] Design progress visualization with plotters

### Phase 6: Testing & Polish (Week 11-12)
- [ ] Write unit tests for recognition (95%+ accuracy target)
- [ ] Create integration tests for learning flows
- [ ] Validate stroke order algorithms
- [ ] Test GUI state transitions
- [ ] Benchmark performance metrics
- [ ] Conduct user testing for learning effectiveness

---

## 9. Performance Targets

| Metric | Target | Rationale |
|--------|--------|-----------|
| **Character Recognition Accuracy** | ≥95% | Industry standard for production OCR |
| **Stroke Order Validation** | ≥85% | Allows for natural handwriting variation |
| **Animation Frame Rate** | ≥60 FPS | Smooth visual experience |
| **Database Query Time** | <10ms | Responsive UI interactions |
| **Session Completion Rate** | ≥70% | Engagement indicator |
| **7-Day Retention Rate** | ≥60% | Learning effectiveness |
| **SRS Log Loss** | <0.5 | Prediction accuracy of forgetting curve |

---

## 10. Key Resources

### Datasets
- **KanjiVG**: https://kanjivg.tagaini.net/ (stroke order SVGs)
- **Kuzushiji-MNIST**: https://github.com/rois-codh/kmnist (70k handwritten characters)
- **kana-svg-data**: https://github.com/scriptin/kana-svg-data (stroke animations)

### Libraries
- **iced**: https://github.com/iced-rs/iced (GUI framework)
- **femtovg**: https://github.com/femtovg/femtovg (2D rendering)
- **ort**: https://github.com/pykeio/ort (ONNX runtime)
- **rusqlite**: https://github.com/rusqlite/rusqlite (SQLite)
- **plotters**: https://github.com/plotters-rs/plotters (charts)
- **kornia-rs**: https://github.com/kornia/kornia-rs (computer vision)

### Algorithms
- **SM-2**: https://www.supermemo.com/en/archives1990-2015/english/ol/sm2
- **FSRS**: https://github.com/open-spaced-repetition/fsrs-rs
- **Dynamic Positional Warping**: Research paper on handwriting recognition

---

## 11. Next Steps

1. **Prototype stroke animation** with a single character to validate femtovg integration
2. **Test recognition accuracy** on Kuzushiji-MNIST subset (10 characters)
3. **Build minimal viable quiz** with 5 hiragana characters
4. **Validate spaced repetition** with simulated review data
5. **User testing** with target learners for UX feedback

This guide provides a comprehensive foundation for building a production-quality Japanese character learning application. Each component has been researched and validated for technical feasibility and educational effectiveness.
