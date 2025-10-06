# Hiragana/Katakana Learning Application: Technical Implementation Guide

## Executive Summary

This document provides a comprehensive technical blueprint for building a Japanese character learning application using Rust and the iced GUI framework. The application combines visual recognition, stroke order animation, handwriting practice, and spaced repetition algorithms to create an effective learning environment for hiragana and katakana.

---

## 1. Architecture Overview

### 1.1 Technology Stack

- **GUI Framework**: iced (Elm architecture)
- **Text Rendering**: cosmic-text (Unicode support)
- **Path Rendering**: lyon (GPU tessellation)
- **Animation**: lilt (via iced's Animation API)
- **Data Storage**: redb or rusqlite
- **Serialization**: serde + bincode/RON/JSON
- **Character Data**: kana-svg-data, animCJK

### 1.2 Application Structure

```
src/
├── main.rs              # Application entry point
├── state/
│   ├── mod.rs           # AppState, SessionState
│   ├── progress.rs      # UserProgress tracking
│   └── navigation.rs    # Screen management
├── ui/
│   ├── mod.rs           # UI components
│   ├── canvas.rs        # Drawing canvas widget
│   ├── flashcard.rs     # Character display
│   └── feedback.rs      # Visual feedback animations
├── models/
│   ├── character.rs     # Character data structures
│   ├── card.rs          # Flashcard model
│   └── review.rs        # Review log
├── persistence/
│   ├── database.rs      # redb/SQLite interface
│   ├── config.rs        # App configuration
│   └── export.rs        # Data export utilities
├── scheduler/
│   ├── sm2.rs           # SM-2 algorithm
│   └── fsrs.rs          # FSRS algorithm (optional)
└── data/
    ├── hiragana.ron     # Hiragana character data
    └── katakana.ron     # Katakana character data
```

---

## 2. Japanese Character Rendering

### 2.1 Font Configuration

Iced uses `cosmic-text` for full Unicode support, including Japanese characters.

**Font Loading:**
```rust
// Load Japanese font at application startup
let font_bytes = include_bytes!("../assets/fonts/NotoSansJP-Regular.otf");
compositor.load_font(Cow::Borrowed(font_bytes));

// Create font reference
let jp_font = Font::with_name("Noto Sans JP");
```

**Text Widget Usage:**
```rust
text("あいうえお")
    .font(Font::with_name("Noto Sans JP"))
    .size(48)
    .shaping(Shaping::Advanced)  // Required for proper Japanese rendering
```

**Recommended Fonts:**
- Noto Sans JP (Google, comprehensive coverage)
- Source Han Sans (Adobe, handwriting-friendly)
- Embedded in binary using `include_bytes!()`

### 2.2 Custom Character Display Widget

Create a specialized widget for flashcard-style character display:

```rust
struct CharacterCard {
    character: char,
    font: Font,
    size: f32,
    show_stroke_order: bool,
}

impl Widget for CharacterCard {
    fn draw(&self, renderer: &mut Renderer, ...) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());

        // Draw character
        frame.fill_text(
            Text {
                content: self.character.to_string(),
                font: self.font,
                size: Pixels(self.size),
                shaping: Shaping::Advanced,
                ..Default::default()
            },
            Point::new(bounds.width / 2.0, bounds.height / 2.0),
            Color::BLACK,
            bounds
        );

        // Optionally overlay stroke order guides
        if self.show_stroke_order {
            self.draw_stroke_guides(&mut frame);
        }

        vec![frame.into_geometry()]
    }
}
```

---

## 3. Stroke Order Animation System

### 3.1 Animation Architecture

Use iced's built-in `Animation` API with `Canvas` widget for smooth, GPU-accelerated stroke animations.

**Core Components:**
```rust
use iced::widget::canvas::{self, Canvas, Frame, Path, Stroke};
use iced::animation::{Animation, Easing};
use std::time::{Duration, Instant};

struct StrokeOrderAnimation {
    strokes: Vec<StrokePath>,
    current_stroke: usize,
    animation: Animation<f32>,
    state: AnimationState,
}

struct StrokePath {
    segments: Vec<PathSegment>,
    total_length: f32,
}

enum AnimationState {
    Idle,
    Playing,
    Paused,
    Complete,
}
```

### 3.2 Animation Implementation

**Sequential Stroke Animation:**
```rust
impl canvas::Program<Message> for StrokeOrderAnimation {
    type State = ();

    fn update(
        &mut self,
        _state: &mut Self::State,
        event: &Event,
        _bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        match event {
            Event::Window(window::Event::RedrawRequested(_)) => {
                let now = Instant::now();

                // Check if current stroke animation completed
                if !self.animation.is_animating(now) {
                    if self.current_stroke < self.strokes.len() - 1 {
                        self.current_stroke += 1;
                        self.animation.go_mut(1.0, now);
                    } else {
                        self.state = AnimationState::Complete;
                    }
                }

                Some(canvas::Action::request_redraw())
            }
            _ => None
        }
    }

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let now = Instant::now();

        // Draw completed strokes
        for i in 0..self.current_stroke {
            frame.stroke(
                &self.strokes[i].full_path,
                Stroke::default()
                    .with_width(3.0)
                    .with_color(Color::BLACK)
            );
        }

        // Draw current stroke with animation
        if let Some(stroke) = self.strokes.get(self.current_stroke) {
            let progress = self.animation.value(now);
            let partial_path = stroke.partial_path(progress);

            frame.stroke(
                &partial_path,
                Stroke::default()
                    .with_width(3.0)
                    .with_color(Color::from_rgb(0.2, 0.5, 0.8))
            );
        }

        vec![frame.into_geometry()]
    }
}
```

**Partial Path Rendering:**
```rust
impl StrokePath {
    fn partial_path(&self, progress: f32) -> Path {
        let target_length = self.total_length * progress;
        let mut accumulated = 0.0;

        Path::new(|builder| {
            for segment in &self.segments {
                if accumulated + segment.length <= target_length {
                    // Draw full segment
                    segment.draw(builder);
                    accumulated += segment.length;
                } else {
                    // Draw partial segment
                    let ratio = (target_length - accumulated) / segment.length;
                    segment.draw_partial(builder, ratio);
                    break;
                }
            }
        })
    }
}
```

### 3.3 Animation Configuration

**Timing Recommendations:**
- Single stroke: 500-800ms
- Pause between strokes: 200-300ms
- Easing: `Easing::EaseOutCubic` (natural writing feel)

```rust
// Initialize animation with proper timing
let animation = Animation::new(0.0)
    .duration(Duration::from_millis(700))
    .easing(Easing::EaseOutCubic)
    .delay(Duration::from_millis(250));
```

### 3.4 SVG Data Integration

**Using KanjiVG/animCJK Data:**
```rust
// Parse SVG path data from kana-svg-data
struct CharacterData {
    char_code: u32,
    strokes: Vec<SvgPath>,
    medians: Vec<Vec<(f32, f32)>>,
}

fn parse_svg_path(path_data: &str) -> Vec<PathSegment> {
    // Parse SVG commands: M (move), C (cubic bezier), S (smooth cubic)
    // Convert to lyon Path segments
    // Store with length calculations for animation
}
```

---

## 4. Handwriting Input & Recognition

### 4.1 Drawing Canvas Implementation

**Canvas State:**
```rust
#[derive(Default)]
struct DrawingState {
    strokes: Vec<Stroke>,
    current_stroke: Option<Stroke>,
    drawing_mode: DrawingMode,
}

struct Stroke {
    points: Vec<Point>,
    timestamp: Instant,
}

enum DrawingMode {
    Freehand,
    StrokeOrder,  // Enforce correct stroke sequence
}
```

**Event Handling:**
```rust
impl canvas::Program<Message> for HandwritingCanvas {
    type State = DrawingState;

    fn update(
        &self,
        state: &mut Self::State,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(pos) = cursor.position_in(bounds) {
                    state.current_stroke = Some(Stroke {
                        points: vec![pos],
                        timestamp: Instant::now(),
                    });
                }
                Some(canvas::Action::request_redraw())
            }

            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if let Some(stroke) = &mut state.current_stroke {
                    if let Some(pos) = cursor.position_in(bounds) {
                        // Add point with distance threshold to reduce noise
                        if let Some(last) = stroke.points.last() {
                            if last.distance(pos) > 2.0 {
                                stroke.points.push(pos);
                            }
                        }
                        return Some(canvas::Action::request_redraw());
                    }
                }
                None
            }

            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                if let Some(stroke) = state.current_stroke.take() {
                    state.strokes.push(stroke);
                    // Trigger recognition
                    Some(canvas::Action::publish(Message::RecognizeCharacter))
                } else {
                    None
                }
            }

            _ => None
        }
    }

    fn draw(
        &self,
        state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());

        // Draw completed strokes
        for stroke in &state.strokes {
            self.draw_stroke(&mut frame, stroke, Color::BLACK);
        }

        // Draw current stroke
        if let Some(stroke) = &state.current_stroke {
            self.draw_stroke(&mut frame, stroke, Color::from_rgb(0.2, 0.5, 0.8));
        }

        vec![frame.into_geometry()]
    }
}

impl HandwritingCanvas {
    fn draw_stroke(&self, frame: &mut Frame, stroke: &Stroke, color: Color) {
        if stroke.points.len() < 2 {
            return;
        }

        let path = Path::new(|builder| {
            if let Some(first) = stroke.points.first() {
                builder.move_to(*first);
                for point in stroke.points.iter().skip(1) {
                    builder.line_to(*point);
                }
            }
        });

        frame.stroke(
            &path,
            Stroke::default()
                .with_width(3.0)
                .with_color(color)
                .with_line_cap(canvas::LineCap::Round)
                .with_line_join(canvas::LineJoin::Round)
        );
    }
}
```

### 4.2 Character Recognition Integration

**Stroke Data Format:**
```rust
// Convert drawing strokes to recognition format
fn to_recognition_format(strokes: &[Stroke]) -> Vec<Vec<Point>> {
    strokes
        .iter()
        .map(|s| s.points.clone())
        .collect()
}
```

**Recognition Workflow:**
1. User completes drawing
2. Normalize stroke coordinates (0-1 range)
3. Pass to recognition algorithm
4. Return top N candidates with confidence scores
5. Validate against expected character

**Recommended Library:**
- `hanzi_lookup` (Rust/WASM) for CJK character recognition
- Fallback: FFI to Python libraries or WASM bridges

---

## 5. Character Data Management

### 5.1 Data Structure

**RON Format (Recommended):**
```rust
// data/hiragana.ron
{
    'あ': CharData(
        unicode: 0x3042,
        romaji: "a",
        hiragana: "あ",
        stroke_count: 3,
        strokes: [
            "M50,30 C45,35 40,45 35,50",
            "M55,40 C60,45 65,55 70,65",
            "M25,70 C35,72 50,70 65,72",
        ],
        medians: [
            [(50, 30), (42, 40), (35, 50)],
            [(55, 40), (62, 50), (70, 65)],
            [(25, 70), (45, 71), (65, 72)],
        ],
        frequency_rank: 1,
    ),
    'い': CharData(
        unicode: 0x3044,
        romaji: "i",
        hiragana: "い",
        stroke_count: 2,
        strokes: [
            "M45,25 C47,35 48,55 49,75",
            "M60,45 C62,50 64,58 65,65",
        ],
        medians: [
            [(45, 25), (47, 50), (49, 75)],
            [(60, 45), (63, 55), (65, 65)],
        ],
        frequency_rank: 2,
    ),
    // ... remaining characters
}
```

**Rust Model:**
```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CharData {
    unicode: u32,
    romaji: String,
    hiragana: String,
    stroke_count: usize,
    strokes: Vec<String>,  // SVG path data
    medians: Vec<Vec<(f32, f32)>>,
    frequency_rank: usize,
}
```

### 5.2 Efficient Lookup System

**Compile-Time Static Map (Recommended for MVP):**
```rust
use phf::phf_map;

static HIRAGANA: phf::Map<char, CharData> = phf_map! {
    'あ' => CharData { /* ... */ },
    'い' => CharData { /* ... */ },
    // ... all hiragana
};

static KATAKANA: phf::Map<char, CharData> = phf_map! {
    'ア' => CharData { /* ... */ },
    'イ' => CharData { /* ... */ },
    // ... all katakana
};

// Zero-cost lookup
fn get_char_data(ch: char) -> Option<&'static CharData> {
    HIRAGANA.get(&ch).or_else(|| KATAKANA.get(&ch))
}
```

**Runtime Loading (For Flexibility):**
```rust
use once_cell::sync::Lazy;
use std::collections::HashMap;

static CHARACTERS: Lazy<HashMap<char, CharData>> = Lazy::new(|| {
    let hiragana_ron = include_str!("../data/hiragana.ron");
    let katakana_ron = include_str!("../data/katakana.ron");

    let mut map = HashMap::new();
    map.extend(ron::from_str::<HashMap<char, CharData>>(hiragana_ron).unwrap());
    map.extend(ron::from_str::<HashMap<char, CharData>>(katakana_ron).unwrap());
    map
});
```

### 5.3 Data Sources

**Pre-Built Datasets:**
- **kana-svg-data** (GitHub: hy2k/kana-svg-data): 177 kana with stroke data
- **animCJK**: 86 hiragana + 91 katakana SVG files
- **strokesvg**: Handwriting-style kana SVGs

**Custom Generation:**
1. Extract SVG paths from source files
2. Calculate median coordinates for each stroke
3. Serialize to RON format
4. Include in binary or load at runtime

---

## 6. Progress Tracking & Persistence

### 6.1 State Management Architecture

**Hybrid Approach:**
```rust
use std::sync::{Arc, RwLock};

struct AppState {
    // UI state (owned, no sharing needed)
    current_screen: Screen,
    session: Option<LearningSession>,

    // Shared progress data (read-heavy)
    progress: Arc<RwLock<UserProgress>>,

    // Database handle
    db: Arc<Database>,
}

struct UserProgress {
    cards: HashMap<char, CardProgress>,
    statistics: Statistics,
    settings: UserSettings,
}

struct CardProgress {
    character: char,
    review_count: u32,
    correct_count: u32,
    last_reviewed: Option<DateTime<Utc>>,
    next_review: DateTime<Utc>,
    ease_factor: f32,
    interval_days: u32,
}
```

### 6.2 Local Storage Implementation

**Dependencies:**
```toml
[dependencies]
redb = "2.0"
serde = { version = "1.0", features = ["derive"] }
bincode = "1.3"
chrono = { version = "0.4", features = ["serde"] }
directories = "5.0"
```

**Database Schema (redb):**
```rust
use redb::{Database, ReadableTable, TableDefinition};

const CARDS_TABLE: TableDefinition<char, &[u8]> = TableDefinition::new("cards");
const REVIEWS_TABLE: TableDefinition<u64, &[u8]> = TableDefinition::new("reviews");
const SETTINGS_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("settings");

struct AppDatabase {
    db: Database,
}

impl AppDatabase {
    fn new() -> Result<Self> {
        let data_dir = directories::ProjectDirs::from("com", "kana-learning", "KanaApp")
            .ok_or("Failed to determine data directory")?;

        let db_path = data_dir.data_dir().join("progress.db");
        std::fs::create_dir_all(data_dir.data_dir())?;

        let db = Database::create(db_path)?;
        Ok(Self { db })
    }

    fn save_card_progress(&self, progress: &CardProgress) -> Result<()> {
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(CARDS_TABLE)?;
            let serialized = bincode::serialize(progress)?;
            table.insert(progress.character, serialized.as_slice())?;
        }
        write_txn.commit()?;
        Ok(())
    }

    fn load_card_progress(&self, character: char) -> Result<Option<CardProgress>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(CARDS_TABLE)?;

        if let Some(data) = table.get(character)? {
            let progress: CardProgress = bincode::deserialize(data.value())?;
            Ok(Some(progress))
        } else {
            Ok(None)
        }
    }

    fn save_review_log(&self, log: &ReviewLog) -> Result<()> {
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(REVIEWS_TABLE)?;
            let id = log.timestamp.timestamp_millis() as u64;
            let serialized = bincode::serialize(log)?;
            table.insert(id, serialized.as_slice())?;
        }
        write_txn.commit()?;
        Ok(())
    }
}
```

**Platform-Specific Paths:**
```rust
use directories::ProjectDirs;

fn get_data_directory() -> PathBuf {
    let proj_dirs = ProjectDirs::from("com", "kana-learning", "KanaApp")
        .expect("Failed to determine data directory");

    // Linux: ~/.local/share/KanaApp/
    // macOS: ~/Library/Application Support/com.kana-learning.KanaApp/
    // Windows: %APPDATA%\kana-learning\KanaApp\

    proj_dirs.data_dir().to_path_buf()
}
```

### 6.3 Statistics & Metrics

**Core Metrics:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Statistics {
    // Accuracy metrics
    total_reviews: u32,
    correct_reviews: u32,
    accuracy_rate: f32,  // correct / total

    // Time tracking
    total_study_time_minutes: u32,
    study_days: HashSet<NaiveDate>,
    current_streak: u32,
    longest_streak: u32,

    // Progress metrics
    cards_mastered: u32,
    cards_learning: u32,
    cards_new: u32,

    // Time-series data
    daily_stats: BTreeMap<NaiveDate, DayStats>,
    weekly_stats: BTreeMap<NaiveDate, WeekStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DayStats {
    date: NaiveDate,
    reviews: u32,
    correct: u32,
    new_cards: u32,
    study_time_minutes: u32,
}
```

**Efficient Aggregation:**
```rust
impl Statistics {
    fn update_after_review(&mut self, character: char, correct: bool, time_spent: Duration) {
        self.total_reviews += 1;
        if correct {
            self.correct_reviews += 1;
        }
        self.accuracy_rate = self.correct_reviews as f32 / self.total_reviews as f32;

        let today = Utc::now().date_naive();
        let day_stats = self.daily_stats.entry(today).or_insert_with(|| DayStats {
            date: today,
            reviews: 0,
            correct: 0,
            new_cards: 0,
            study_time_minutes: 0,
        });

        day_stats.reviews += 1;
        if correct {
            day_stats.correct += 1;
        }
        day_stats.study_time_minutes += time_spent.as_secs() as u32 / 60;

        self.update_streak(today);
    }

    fn update_streak(&mut self, today: NaiveDate) {
        if !self.study_days.contains(&today) {
            self.study_days.insert(today);

            let yesterday = today - chrono::Duration::days(1);
            if self.study_days.contains(&yesterday) {
                self.current_streak += 1;
            } else {
                self.current_streak = 1;
            }

            self.longest_streak = self.longest_streak.max(self.current_streak);
        }
    }
}
```

---

## 7. Spaced Repetition System

### 7.1 SM-2 Algorithm (Recommended for MVP)

**Algorithm Implementation:**
```rust
const MIN_EF: f32 = 1.3;
const INITIAL_EF: f32 = 2.5;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SM2Card {
    character: char,
    ease_factor: f32,
    interval_days: u32,
    repetitions: u32,
    next_review: DateTime<Utc>,
}

impl SM2Card {
    fn new(character: char) -> Self {
        Self {
            character,
            ease_factor: INITIAL_EF,
            interval_days: 0,
            repetitions: 0,
            next_review: Utc::now(),
        }
    }

    fn review(&mut self, quality: u8) {
        // quality: 0-5 rating
        // 5: perfect response
        // 4: correct after hesitation
        // 3: correct with difficulty
        // 2: incorrect but remembered
        // 1: incorrect but familiar
        // 0: complete blackout

        if quality < 3 {
            // Reset learning
            self.repetitions = 0;
            self.interval_days = 1;
        } else {
            // Update ease factor
            let q = quality as f32;
            self.ease_factor = (self.ease_factor + 0.1 - (5.0 - q) * (0.08 + (5.0 - q) * 0.02))
                .max(MIN_EF);

            // Calculate next interval
            self.interval_days = match self.repetitions {
                0 => 1,
                1 => 6,
                _ => (self.interval_days as f32 * self.ease_factor).round() as u32,
            };

            self.repetitions += 1;
        }

        // Schedule next review
        self.next_review = Utc::now() + chrono::Duration::days(self.interval_days as i64);
    }
}
```

**Queue Management:**
```rust
#[derive(Debug)]
struct ReviewQueue {
    new_cards: Vec<char>,
    learning_cards: Vec<SM2Card>,
    review_cards: Vec<SM2Card>,
    daily_new_limit: usize,
    daily_review_limit: usize,
}

impl ReviewQueue {
    fn get_next_card(&mut self) -> Option<ReviewCard> {
        let now = Utc::now();

        // Priority 1: Learning cards (interval < 1 day)
        if let Some(idx) = self.learning_cards.iter()
            .position(|card| card.next_review <= now) {
            return Some(ReviewCard::Learning(self.learning_cards.remove(idx)));
        }

        // Priority 2: Review cards (due for review)
        if let Some(idx) = self.review_cards.iter()
            .position(|card| card.next_review <= now) {
            return Some(ReviewCard::Review(self.review_cards.remove(idx)));
        }

        // Priority 3: New cards (within daily limit)
        if !self.new_cards.is_empty() {
            let character = self.new_cards.remove(0);
            return Some(ReviewCard::New(SM2Card::new(character)));
        }

        None
    }
}
```

### 7.2 FSRS Algorithm (Advanced)

**For future enhancement after collecting 1000+ reviews:**
```rust
// FSRS (Free Spaced Repetition Scheduler)
// 20-30% more efficient than SM-2

#[derive(Debug, Clone)]
struct FSRSCard {
    character: char,
    difficulty: f32,    // D: How hard this card is to remember
    stability: f32,     // S: How long memory lasts
    retrievability: f32, // R: Current recall probability
    last_review: DateTime<Utc>,
}

impl FSRSCard {
    fn retrievability(&self, elapsed_days: f32) -> f32 {
        // R(t,S) = (1 + F·t/S)^C
        // Where F and C are tuned parameters
        let f = 0.9;  // Forgetting curve parameter
        let c = -0.5; // Curvature parameter
        (1.0 + f * elapsed_days / self.stability).powf(c)
    }
}
```

**Recommendation:** Start with SM-2, collect review data, migrate to FSRS once sufficient data is available.

---

## 8. Application State Management (Elm Architecture)

### 8.1 State Structure

```rust
#[derive(Debug)]
struct AppState {
    screen: Screen,
    session_data: Option<SessionData>,
    progress: Arc<RwLock<UserProgress>>,
    db: Arc<AppDatabase>,
}

#[derive(Debug)]
enum Screen {
    MainMenu(MenuState),
    CharacterList(ListState),
    Learning(LearningSession),
    Practice(PracticeSession),
    Review(ReviewSession),
    Statistics(StatsScreen),
}

#[derive(Debug)]
struct LearningSession {
    queue: ReviewQueue,
    current_card: Option<SM2Card>,
    card_index: usize,
    total_cards: usize,
    session_history: Vec<ReviewResult>,
    session_start: Instant,
}

#[derive(Debug)]
struct PracticeSession {
    character: char,
    char_data: CharData,
    mode: PracticeMode,
    animation_state: StrokeOrderAnimation,
    drawing_state: DrawingState,
    attempts: u32,
}

#[derive(Debug)]
enum PracticeMode {
    WatchAnimation,
    TraceWithGuide,
    TraceWithoutGuide,
    FreeDrawing,
}
```

### 8.2 Message Architecture

```rust
#[derive(Debug, Clone)]
enum Message {
    // Navigation
    NavigateToMenu,
    NavigateToList,
    NavigateToLearning,
    NavigateToPractice(char),
    NavigateToStats,

    // Learning session
    StartSession,
    SubmitAnswer { character: char, quality: u8 },
    NextCard,
    EndSession,

    // Practice session
    StartAnimation,
    PauseAnimation,
    ClearCanvas,
    RecognizeDrawing,
    RecognitionResult { matches: Vec<(char, f32)> },
    NextPracticeMode,

    // Drawing events (delegated to canvas)
    DrawingEvent(canvas::Event),

    // Statistics
    LoadStatistics,
    StatisticsLoaded(Statistics),

    // Persistence
    SaveProgress,
    ProgressSaved,
    LoadProgress,
    ProgressLoaded(UserProgress),
}
```

### 8.3 Update Function

```rust
impl Application for App {
    type Message = Message;
    type Executor = executor::Default;
    type Flags = ();
    type Theme = Theme;

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::NavigateToLearning => {
                let queue = self.build_review_queue();
                self.state.screen = Screen::Learning(LearningSession {
                    queue,
                    current_card: None,
                    card_index: 0,
                    total_cards: 0,
                    session_history: Vec::new(),
                    session_start: Instant::now(),
                });
                self.update(Message::NextCard)
            }

            Message::SubmitAnswer { character, quality } => {
                if let Screen::Learning(session) = &mut self.state.screen {
                    if let Some(card) = &mut session.current_card {
                        // Update SM-2 algorithm
                        card.review(quality);

                        // Record review
                        let result = ReviewResult {
                            character,
                            quality,
                            timestamp: Utc::now(),
                        };
                        session.session_history.push(result.clone());

                        // Update progress
                        let mut progress = self.state.progress.write().unwrap();
                        progress.record_review(result);

                        // Save to database
                        let db = self.state.db.clone();
                        let card_clone = card.clone();
                        return Task::perform(
                            async move {
                                db.save_card_progress(&card_clone).ok();
                            },
                            |_| Message::NextCard
                        );
                    }
                }
                Task::none()
            }

            Message::NextCard => {
                if let Screen::Learning(session) = &mut self.state.screen {
                    session.current_card = session.queue.get_next_card();

                    if session.current_card.is_some() {
                        session.card_index += 1;
                        Task::none()
                    } else {
                        // Session complete
                        self.update(Message::EndSession)
                    }
                } else {
                    Task::none()
                }
            }

            Message::NavigateToPractice(character) => {
                let char_data = get_char_data(character).cloned().unwrap();

                self.state.screen = Screen::Practice(PracticeSession {
                    character,
                    char_data: char_data.clone(),
                    mode: PracticeMode::WatchAnimation,
                    animation_state: StrokeOrderAnimation::new(char_data.strokes),
                    drawing_state: DrawingState::default(),
                    attempts: 0,
                });

                self.update(Message::StartAnimation)
            }

            Message::RecognizeDrawing => {
                if let Screen::Practice(session) = &mut self.state.screen {
                    let strokes = session.drawing_state.strokes.clone();

                    // Async recognition
                    return Task::perform(
                        async move {
                            // Call recognition engine
                            recognize_character(strokes).await
                        },
                        |matches| Message::RecognitionResult { matches }
                    );
                }
                Task::none()
            }

            Message::RecognitionResult { matches } => {
                if let Screen::Practice(session) = &mut self.state.screen {
                    if let Some((recognized, confidence)) = matches.first() {
                        if *recognized == session.character && *confidence > 0.7 {
                            // Success!
                            // Show positive feedback, advance to next mode
                            session.mode = match session.mode {
                                PracticeMode::WatchAnimation => PracticeMode::TraceWithGuide,
                                PracticeMode::TraceWithGuide => PracticeMode::TraceWithoutGuide,
                                PracticeMode::TraceWithoutGuide => PracticeMode::FreeDrawing,
                                PracticeMode::FreeDrawing => {
                                    // Completed all modes
                                    return self.update(Message::NavigateToMenu);
                                }
                            };
                        } else {
                            // Incorrect, provide feedback
                            session.attempts += 1;
                        }
                    }
                }
                Task::none()
            }

            _ => Task::none()
        }
    }

    fn view(&self) -> Element<Message> {
        match &self.state.screen {
            Screen::MainMenu(menu) => self.view_menu(menu),
            Screen::Learning(session) => self.view_learning(session),
            Screen::Practice(session) => self.view_practice(session),
            Screen::Statistics(stats) => self.view_statistics(stats),
            _ => container(text("Loading...")).into(),
        }
    }
}
```

---

## 9. Educational UX Patterns

### 9.1 Progressive Learning Modes

**Mode Progression:**
1. **Watch Animation** → Learn stroke order passively
2. **Trace With Guide** → Ghosted character overlay
3. **Trace Without Guide** → Stroke order hints only
4. **Free Drawing** → Full recall test

```rust
enum PracticeMode {
    WatchAnimation,
    TraceWithGuide,       // Ghost outline + stroke numbers
    TraceWithoutGuide,    // Stroke numbers only
    FreeDrawing,          // No assistance
}
```

### 9.2 Visual Feedback System

**Immediate Feedback (200-500ms):**
```rust
struct FeedbackAnimation {
    feedback_type: FeedbackType,
    animation: Animation<f32>,
    color: Color,
}

enum FeedbackType {
    Correct,      // Green flash + checkmark
    Incorrect,    // Red shake + X
    Partial,      // Yellow pulse
}

impl FeedbackAnimation {
    fn new(feedback_type: FeedbackType) -> Self {
        let (duration, color) = match feedback_type {
            FeedbackType::Correct => (
                Duration::from_millis(400),
                Color::from_rgb(0.2, 0.8, 0.3)
            ),
            FeedbackType::Incorrect => (
                Duration::from_millis(300),
                Color::from_rgb(0.9, 0.2, 0.2)
            ),
            FeedbackType::Partial => (
                Duration::from_millis(500),
                Color::from_rgb(0.9, 0.7, 0.2)
            ),
        };

        Self {
            feedback_type,
            animation: Animation::new(0.0)
                .duration(duration)
                .easing(Easing::EaseOutElastic),
            color,
        }
    }
}
```

**Shake Animation for Errors:**
```rust
fn draw_with_shake(frame: &mut Frame, animation: &Animation<f32>, offset: Point) {
    let now = Instant::now();
    let progress = animation.value(now);

    // Shake oscillation
    let shake_x = (progress * 10.0 * std::f32::consts::PI).sin() * 5.0 * (1.0 - progress);
    let shake_offset = Point::new(shake_x, 0.0);

    // Draw with offset
    // ...
}
```

### 9.3 Progress Visualization

**Circular Progress Indicator:**
```rust
fn draw_progress_circle(
    frame: &mut Frame,
    center: Point,
    radius: f32,
    progress: f32,  // 0.0 to 1.0
) {
    // Background circle
    frame.fill(
        &Path::circle(center, radius),
        Color::from_rgba(0.5, 0.5, 0.5, 0.2)
    );

    // Progress arc
    let end_angle = progress * 2.0 * std::f32::consts::PI;
    let arc = Path::new(|builder| {
        builder.arc(canvas::path::Arc {
            center,
            radius,
            start_angle: -std::f32::consts::FRAC_PI_2,
            end_angle: -std::f32::consts::FRAC_PI_2 + end_angle,
        });
    });

    frame.stroke(
        &arc,
        Stroke::default()
            .with_width(4.0)
            .with_color(Color::from_rgb(0.2, 0.6, 0.9))
            .with_line_cap(canvas::LineCap::Round)
    );

    // Center text
    frame.fill_text(
        Text {
            content: format!("{}%", (progress * 100.0) as u32),
            size: Pixels(14.0),
            ..Default::default()
        },
        center,
        Color::BLACK,
        Rectangle::new(center, Size::new(50.0, 20.0))
    );
}
```

### 9.4 Gamification Elements

**Achievement System:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Achievement {
    id: String,
    title: String,
    description: String,
    icon: String,
    unlocked: bool,
    unlock_date: Option<DateTime<Utc>>,
}

const ACHIEVEMENTS: &[Achievement] = &[
    Achievement {
        id: "first_character",
        title: "First Steps",
        description: "Learn your first character",
        icon: "🎯",
        unlocked: false,
        unlock_date: None,
    },
    Achievement {
        id: "hiragana_complete",
        title: "Hiragana Master",
        description: "Complete all 46 hiragana characters",
        icon: "🏆",
        unlocked: false,
        unlock_date: None,
    },
    Achievement {
        id: "streak_7",
        title: "Week Warrior",
        description: "Study for 7 days in a row",
        icon: "🔥",
        unlocked: false,
        unlock_date: None,
    },
];
```

**Streak Tracking:**
```rust
struct StreakDisplay {
    current_streak: u32,
    longest_streak: u32,
    flame_animation: Animation<f32>,
}

impl StreakDisplay {
    fn view(&self) -> Element<Message> {
        row![
            text("🔥").size(32),
            column![
                text(format!("{} day streak", self.current_streak))
                    .size(18),
                text(format!("Best: {} days", self.longest_streak))
                    .size(12)
                    .color(Color::from_rgb(0.5, 0.5, 0.5))
            ]
        ]
        .spacing(10)
        .into()
    }
}
```

---

## 10. Accessibility & Design Guidelines

### 10.1 Color Contrast (WCAG Compliance)

**Minimum Standards:**
- Normal text: 4.5:1 contrast (WCAG AA)
- Large text (18pt+): 3:1 contrast
- Enhanced: 7:1 for normal text (WCAG AAA)

```rust
// High-contrast color palette
const BG_COLOR: Color = Color::from_rgb(0.95, 0.95, 0.97);  // Light gray
const TEXT_COLOR: Color = Color::from_rgb(0.1, 0.1, 0.15);  // Dark gray
const ACCENT_COLOR: Color = Color::from_rgb(0.2, 0.5, 0.8); // Blue
const SUCCESS_COLOR: Color = Color::from_rgb(0.2, 0.7, 0.3); // Green
const ERROR_COLOR: Color = Color::from_rgb(0.85, 0.2, 0.2); // Red
```

**Don't Rely on Color Alone:**
```rust
// Bad: Color-only feedback
frame.stroke(&path, Stroke::default().with_color(success_color));

// Good: Color + icon/text
row![
    text("✓").size(24).color(success_color),
    text("Correct!").color(success_color)
]
```

### 10.2 Typography

**Font Sizing:**
- Character display: 48-72px
- Primary text: 16-18px
- Secondary text: 14px
- Minimum: 12px

**Japanese Text:**
```rust
text(character)
    .font(Font::with_name("Noto Sans JP"))
    .size(64)
    .shaping(Shaping::Advanced)
```

### 10.3 Touch-Friendly Design

**Minimum Target Sizes:**
- Buttons: 44x44 pixels (iOS), 48x48 pixels (Android)
- Touch areas: 8-10mm (approximately 48px)

```rust
button("Next")
    .padding(12)  // Ensures adequate touch target
    .width(Length::Fixed(120.0))
    .height(Length::Fixed(48.0))
```

---

## 11. Implementation Roadmap

### Phase 1: MVP (2-4 weeks)
- [x] Basic iced application structure
- [ ] Character data loading (hiragana only)
- [ ] Simple flashcard display
- [ ] SM-2 spaced repetition
- [ ] Progress persistence (redb)
- [ ] Basic statistics

### Phase 2: Core Features (3-5 weeks)
- [ ] Stroke order animation
- [ ] Drawing canvas with stroke capture
- [ ] Visual feedback system
- [ ] Katakana support
- [ ] Enhanced UI/navigation
- [ ] Achievement system

### Phase 3: Advanced Features (4-6 weeks)
- [ ] Character recognition integration
- [ ] Stroke order validation
- [ ] Progressive practice modes
- [ ] Advanced statistics & graphs
- [ ] FSRS algorithm (optional)
- [ ] Export/import functionality

### Phase 4: Polish (2-3 weeks)
- [ ] Accessibility improvements
- [ ] Sound effects
- [ ] Themes/customization
- [ ] Performance optimization
- [ ] Testing & bug fixes
- [ ] Documentation

---

## 12. Performance Considerations

### 12.1 Rendering Optimization

**Canvas Caching:**
```rust
impl canvas::Program for MyCanvas {
    fn draw(&self, state: &Self::State, ...) -> Vec<Geometry> {
        // Cache static elements
        let cached_bg = self.cache.draw(renderer, bounds.size(), |frame| {
            // Draw static background/grid
            self.draw_background(frame);
        });

        // Draw dynamic elements
        let mut frame = Frame::new(renderer, bounds.size());
        self.draw_strokes(&mut frame, state);

        vec![cached_bg, frame.into_geometry()]
    }
}
```

### 12.2 Database Performance

**Batch Operations:**
```rust
// Bad: Individual transactions
for card in cards {
    db.save_card_progress(card)?;
}

// Good: Single transaction
let write_txn = db.begin_write()?;
{
    let mut table = write_txn.open_table(CARDS_TABLE)?;
    for card in cards {
        let serialized = bincode::serialize(card)?;
        table.insert(card.character, serialized.as_slice())?;
    }
}
write_txn.commit()?;
```

### 12.3 Memory Management

**Lazy Loading:**
- Load character data on-demand for large datasets
- Use `Arc<T>` for shared read-only data
- Implement pagination for long lists

---

## 13. Testing Strategy

### 13.1 Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sm2_algorithm() {
        let mut card = SM2Card::new('あ');

        // Perfect recall
        card.review(5);
        assert_eq!(card.interval_days, 1);
        assert_eq!(card.ease_factor, 2.6);

        card.review(5);
        assert_eq!(card.interval_days, 6);

        card.review(5);
        assert_eq!(card.interval_days, 15);

        // Failure resets
        card.review(2);
        assert_eq!(card.interval_days, 1);
        assert_eq!(card.repetitions, 0);
    }

    #[test]
    fn test_review_queue_priority() {
        let mut queue = ReviewQueue::new();

        // Add cards with different states
        let learning = SM2Card {
            character: 'あ',
            next_review: Utc::now() - chrono::Duration::hours(1),
            interval_days: 0,
            ..Default::default()
        };

        let review = SM2Card {
            character: 'い',
            next_review: Utc::now() - chrono::Duration::hours(1),
            interval_days: 7,
            ..Default::default()
        };

        queue.learning_cards.push(learning);
        queue.review_cards.push(review);
        queue.new_cards.push('う');

        // Should prioritize learning over review over new
        let next = queue.get_next_card().unwrap();
        assert_eq!(next.character(), 'あ');
    }
}
```

### 13.2 Integration Tests

```rust
#[tokio::test]
async fn test_full_learning_session() {
    let db = AppDatabase::new_temp().unwrap();
    let mut session = LearningSession::new(db.clone());

    // Start session
    session.start();
    assert!(session.current_card.is_some());

    // Complete 10 reviews
    for _ in 0..10 {
        let quality = 4; // Good response
        session.submit_answer(quality).await.unwrap();
        session.next_card();
    }

    // Verify progress saved
    let progress = db.load_all_progress().unwrap();
    assert_eq!(progress.len(), 10);
}
```

---

## 14. Deployment & Distribution

### 14.1 Build Configuration

**Cargo.toml:**
```toml
[package]
name = "kana-learning"
version = "0.1.0"
edition = "2021"

[dependencies]
iced = { version = "0.13", features = ["canvas", "tokio", "advanced"] }
redb = "2.0"
serde = { version = "1.0", features = ["derive"] }
bincode = "1.3"
chrono = { version = "0.4", features = ["serde"] }
directories = "5.0"
phf = { version = "0.11", features = ["macros"] }
ron = "0.8"

[profile.release]
opt-level = 3
lto = true
codegen-units = 1
strip = true
```

### 14.2 Platform Packaging

**Linux (AppImage):**
```bash
cargo build --release
# Use linuxdeploy or similar
```

**macOS (Bundle):**
```bash
cargo bundle --release
```

**Windows (Installer):**
```bash
cargo build --release
# Use WiX or NSIS
```

---

## 15. Resources & References

### Documentation
- **Iced Guide**: https://book.iced.rs/
- **Iced API Docs**: https://docs.rs/iced/
- **Lyon Path**: https://docs.rs/lyon/

### Character Data
- **kana-svg-data**: https://github.com/hy2k/kana-svg-data
- **animCJK**: https://github.com/parsimonhi/animCJK
- **KanjiVG**: https://kanjivg.tagaini.net/

### Algorithms
- **SM-2 Algorithm**: https://www.supermemo.com/en/blog/application-of-a-computer-to-improve-the-results-obtained-in-working-with-the-supermemo-method
- **FSRS**: https://github.com/open-spaced-repetition/fsrs-rs

### Libraries
- **redb**: https://github.com/cberner/redb
- **phf**: https://github.com/rust-phf/rust-phf
- **hanzi_lookup**: https://github.com/gugray/hanzi_lookup

---

## Conclusion

This comprehensive guide provides the technical foundation for building a production-ready hiragana/katakana learning application. The architecture leverages Rust's performance and safety features with iced's modern GUI framework to deliver a smooth, educational user experience.

**Key Success Factors:**
1. **Start Simple**: Implement SM-2 MVP before advanced features
2. **Iterate Based on Data**: Collect review logs to optimize algorithms
3. **Prioritize UX**: Fast feedback, smooth animations, clear progress
4. **Plan for Scale**: Use efficient data structures from the start
5. **Test Thoroughly**: Especially SRS algorithm and persistence layer

The modular design allows for incremental development while maintaining code quality and performance. Each component—rendering, animation, recognition, persistence, and scheduling—can be developed and tested independently before integration.
