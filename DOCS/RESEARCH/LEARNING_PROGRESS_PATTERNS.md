# Learning Progress Tracking Patterns in Rust

## Overview
This document analyzes patterns and best practices for implementing learning progress tracking in Rust applications, covering data persistence, spaced repetition algorithms, user statistics, and progress visualization.

---

## 1. Data Persistence Solutions

### 1.1 SQLite with Rusqlite
**Crate**: `rusqlite` (v0.37.0+ with SQLite 3.50.2)

**Key Features**:
- Ergonomic Rust wrapper for SQLite
- In-memory and file-based database support
- Type-safe parameter binding and query mapping
- Transaction support, extensions, and tracing
- Zero setup, fully embedded

**Common Pattern**:
```rust
use rusqlite::{Connection, Result};

fn create_progress_db() -> Result<()> {
    let conn = Connection::open("learning_progress.db")?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS user_progress (
            id INTEGER PRIMARY KEY,
            user_id TEXT NOT NULL,
            item_id TEXT NOT NULL,
            mastery_level INTEGER,
            review_count INTEGER,
            last_reviewed TIMESTAMP,
            next_review TIMESTAMP,
            easiness_factor REAL,
            interval_days INTEGER
        )",
        (),
    )?;

    Ok(())
}
```

**Best Practices**:
- Use prepared statements for repeated queries
- Implement transactions for batch updates
- Leverage SQLite's datetime functions for scheduling
- Store performance metrics in normalized tables

**Use Cases**:
- Tracking review history and intervals
- Storing user performance metrics
- Maintaining flashcard deck state
- Recording session statistics

### 1.2 JSON File Persistence with Serde
**Crates**: `serde`, `serde_json`

**Key Features**:
- Zero-boilerplate serialization with derive macros
- Strong type safety at compile time
- Human-readable format for configuration
- No runtime reflection overhead

**Common Pattern**:
```rust
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Serialize, Deserialize, Debug)]
struct UserProgress {
    user_id: String,
    items_mastered: Vec<String>,
    current_streak: u32,
    total_reviews: u32,
    accuracy_rate: f32,
}

fn save_progress(progress: &UserProgress) -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::to_string_pretty(progress)?;
    fs::write("user_progress.json", json)?;
    Ok(())
}

fn load_progress() -> Result<UserProgress, Box<dyn std::error::Error>> {
    let data = fs::read_to_string("user_progress.json")?;
    let progress = serde_json::from_str(&data)?;
    Ok(progress)
}
```

**Best Practices**:
- Use `to_string_pretty` for human-readable output
- Implement error handling with `Result` types
- Consider file locking for concurrent access
- Backup data before writes

**Use Cases**:
- Configuration and settings storage
- Simple progress snapshots
- Export/import functionality
- Lightweight applications without database needs

---

## 2. Spaced Repetition Algorithms

### 2.1 SM-2 Algorithm (SuperMemo 2)
**Crate**: `spaced-repetition` (v1.1.0)

**Key Features**:
- Inspired by Anki and SuperMemo
- Optional Serde serialization
- Lightweight (40KB, 734 lines)
- Uses easiness factor and interval scheduling

**Core Concepts**:
- **Easiness Factor (EF)**: Determines how quickly intervals increase (default: 2.5)
- **Interval**: Days until next review
- **Repetitions**: Number of successful reviews

**Common Pattern**:
```rust
struct Card {
    id: String,
    easiness_factor: f32,  // Default: 2.5
    interval: u32,         // Days
    repetitions: u32,      // Success count
    next_review: DateTime<Utc>,
}

fn update_card_sm2(card: &mut Card, quality: u8) {
    // quality: 0-5 where 0=complete blackout, 5=perfect recall

    if quality >= 3 {
        // Correct response
        if card.repetitions == 0 {
            card.interval = 1;
        } else if card.repetitions == 1 {
            card.interval = 6;
        } else {
            card.interval = (card.interval as f32 * card.easiness_factor) as u32;
        }
        card.repetitions += 1;
    } else {
        // Incorrect response - restart
        card.repetitions = 0;
        card.interval = 1;
    }

    // Update easiness factor
    card.easiness_factor = (card.easiness_factor +
        (0.1 - (5.0 - quality as f32) * (0.08 + (5.0 - quality as f32) * 0.02)))
        .max(1.3);

    card.next_review = Utc::now() + Duration::days(card.interval as i64);
}
```

### 2.2 FSRS Algorithm (Free Spaced Repetition Scheduler)
**Note**: Newer algorithm used in modern Anki versions

**Key Differences**:
- More sophisticated than SM-2
- Better handling of forgetting curves
- Adaptive to individual learner patterns

**Integration Point**:
- Anki's Rust implementation in `rslib/src/scheduler/states`
- Consider for advanced applications

---

## 3. User Statistics and Performance Tracking

### 3.1 Metrics Collection
**Crate**: `metrics` (latest)

**Key Concepts**:
Three fundamental metric types:

1. **Counters**: Monotonically increasing values
   - Total reviews completed
   - Total items learned
   - Correct/incorrect answers

2. **Gauges**: Values that can increase or decrease
   - Current streak
   - Active cards in review
   - Current mastery level

3. **Histograms**: Statistical analysis over observations
   - Response time distribution
   - Accuracy rate over time
   - Session duration patterns

**Common Pattern**:
```rust
use metrics::{counter, gauge, histogram};

struct LearningMetrics {
    total_reviews: u64,
    current_streak: i32,
    response_times: Vec<f64>,
}

impl LearningMetrics {
    fn record_review(&mut self, correct: bool, response_time_ms: f64) {
        counter!("reviews.total").increment(1);

        if correct {
            counter!("reviews.correct").increment(1);
            self.current_streak += 1;
        } else {
            counter!("reviews.incorrect").increment(1);
            self.current_streak = 0;
        }

        gauge!("streak.current").set(self.current_streak as f64);
        histogram!("response.time").record(response_time_ms);
    }
}
```

### 3.2 Mastery Level Tracking

**Common Implementation Pattern**:
```rust
#[derive(Debug, Clone)]
enum MasteryLevel {
    Unknown,      // Not yet studied
    Learning,     // In initial learning phase
    Young,        // Recently learned (< 21 days)
    Mature,       // Well-learned (21+ days)
    Mastered,     // Consistently correct (90%+ accuracy)
}

struct ItemProgress {
    item_id: String,
    mastery_level: MasteryLevel,
    total_reviews: u32,
    correct_reviews: u32,
    interval_days: u32,
    last_accuracy: f32,
}

impl ItemProgress {
    fn calculate_mastery(&mut self) {
        let accuracy = self.correct_reviews as f32 / self.total_reviews as f32;

        self.mastery_level = match (accuracy, self.interval_days) {
            (acc, _) if self.total_reviews < 3 => MasteryLevel::Learning,
            (acc, days) if acc >= 0.9 && days >= 21 => MasteryLevel::Mastered,
            (_, days) if days >= 21 => MasteryLevel::Mature,
            (_, days) if days >= 1 => MasteryLevel::Young,
            _ => MasteryLevel::Learning,
        };
    }
}
```

### 3.3 Learning Analytics

**Key Metrics to Track**:
- **Retention Rate**: Percentage of items retained over time
- **Review Velocity**: Reviews per day/week/month
- **Time to Mastery**: Days from first review to mastery
- **Accuracy Trends**: Accuracy changes over time
- **Session Patterns**: Study time distribution

**Storage Pattern**:
```rust
struct LearningAnalytics {
    user_id: String,
    date_range: (DateTime<Utc>, DateTime<Utc>),
    total_items: u32,
    items_mastered: u32,
    retention_rate: f32,
    avg_reviews_per_day: f32,
    avg_accuracy: f32,
    total_study_time_minutes: u32,
}
```

---

## 4. Progress Visualization

### 4.1 Terminal UI with Ratatui
**Crate**: `ratatui` (actively maintained fork of tui-rs)

**Key Features**:
- Pre-built widgets: tables, charts, lists, progress bars
- Real-time data visualization
- Dynamic charts and tables
- Responsive terminal layouts

**Available Widgets**:
- **Paragraph**: Text display with styling
- **Block**: Container with borders and titles
- **Table**: Structured data display
- **List**: Scrollable item lists
- **Gauge**: Progress indicators
- **BarChart**: Bar graph visualization
- **Sparkline**: Compact line charts

**Common Pattern**:
```rust
use ratatui::{
    backend::CrosstermBackend,
    widgets::{Block, Borders, Gauge, List, ListItem},
    Terminal,
};

fn render_progress_ui(progress: &UserProgress) {
    let mut terminal = Terminal::new(CrosstermBackend::new(std::io::stdout()))?;

    terminal.draw(|f| {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),  // Title
                Constraint::Length(3),  // Progress bar
                Constraint::Min(0),     // Stats list
            ])
            .split(f.area());

        // Progress gauge
        let gauge = Gauge::default()
            .block(Block::default().title("Mastery Progress").borders(Borders::ALL))
            .gauge_style(Style::default().fg(Color::Green))
            .ratio(progress.items_mastered as f64 / progress.total_items as f64);
        f.render_widget(gauge, chunks[1]);

        // Statistics list
        let items: Vec<ListItem> = vec![
            ListItem::new(format!("Current Streak: {} days", progress.current_streak)),
            ListItem::new(format!("Accuracy: {:.1}%", progress.accuracy_rate * 100.0)),
            ListItem::new(format!("Total Reviews: {}", progress.total_reviews)),
        ];
        let list = List::new(items)
            .block(Block::default().title("Statistics").borders(Borders::ALL));
        f.render_widget(list, chunks[2]);
    })?;
}
```

**Additional TUI Crates**:
- `tui-bar-graph`: Dedicated bar graph widget
- `crossterm`: Terminal manipulation backend

### 4.2 Charts and Graphs with Plotters
**Crate**: `plotters` (v0.3+)

**Key Features**:
- Multiple backends: bitmap, vector, WASM, GTK/Cairo
- Chart types: line, scatter, histogram, candlestick, pie
- Real-time data visualization
- High performance for large datasets

**Common Pattern**:
```rust
use plotters::prelude::*;

fn plot_learning_curve(review_data: &[(DateTime<Utc>, f32)]) -> Result<(), Box<dyn std::error::Error>> {
    let root = BitMapBackend::new("learning_curve.png", (800, 600)).into_drawing_area();
    root.fill(&WHITE)?;

    let (min_date, max_date) = review_data.iter()
        .map(|(d, _)| d)
        .fold((review_data[0].0, review_data[0].0), |(min, max), d| {
            (min.min(*d), max.max(*d))
        });

    let mut chart = ChartBuilder::on(&root)
        .caption("Learning Progress Over Time", ("sans-serif", 50))
        .margin(10)
        .x_label_area_size(30)
        .y_label_area_size(40)
        .build_cartesian_2d(min_date..max_date, 0.0f32..100.0f32)?;

    chart.configure_mesh().draw()?;

    chart.draw_series(LineSeries::new(
        review_data.iter().map(|(date, accuracy)| (*date, *accuracy * 100.0)),
        &RED,
    ))?;

    root.present()?;
    Ok(())
}
```

**Visualization Types for Learning Apps**:
- **Line Charts**: Accuracy trends over time
- **Bar Charts**: Reviews per day/week
- **Histograms**: Response time distribution
- **Heatmaps**: Study patterns (time of day/week)
- **Scatter Plots**: Correlation between study time and retention

### 4.3 Alternative Visualization Libraries

**Charming**:
- Built on Apache ECharts
- Declarative API
- Rich interactive charts

**Plotlib**:
- Simple API for basic plots
- Histograms, scatter plots, bar charts
- Lightweight alternative

---

## 5. Complete Implementation Example

### 5.1 Architecture Overview
```
┌─────────────────────────────────────────┐
│         User Interface Layer            │
│    (Ratatui TUI / CLI / Web)           │
└─────────────────────────────────────────┘
                    ↓
┌─────────────────────────────────────────┐
│       Learning Logic Layer              │
│  - Spaced Repetition (SM-2/FSRS)       │
│  - Mastery Calculation                  │
│  - Review Scheduling                    │
└─────────────────────────────────────────┘
                    ↓
┌─────────────────────────────────────────┐
│      Metrics & Analytics Layer          │
│  - Statistics Tracking                  │
│  - Performance Metrics                  │
│  - Progress Calculation                 │
└─────────────────────────────────────────┘
                    ↓
┌─────────────────────────────────────────┐
│       Data Persistence Layer            │
│  - SQLite (rusqlite)                    │
│  - JSON (serde_json)                    │
└─────────────────────────────────────────┘
```

### 5.2 Core Data Structures
```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LearningItem {
    id: String,
    content: String,
    category: String,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ReviewRecord {
    item_id: String,
    reviewed_at: DateTime<Utc>,
    quality: u8,           // 0-5 rating
    response_time_ms: u64,
    was_correct: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CardState {
    item_id: String,
    easiness_factor: f32,
    interval_days: u32,
    repetitions: u32,
    next_review: DateTime<Utc>,
    mastery_level: MasteryLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UserStats {
    user_id: String,
    total_reviews: u32,
    correct_reviews: u32,
    current_streak: u32,
    longest_streak: u32,
    total_study_time_ms: u64,
    items_mastered: u32,
    last_session: DateTime<Utc>,
}
```

### 5.3 Database Schema
```sql
-- Learning items (flashcards, words, etc.)
CREATE TABLE items (
    id TEXT PRIMARY KEY,
    content TEXT NOT NULL,
    category TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Card state for spaced repetition
CREATE TABLE card_states (
    item_id TEXT PRIMARY KEY,
    easiness_factor REAL DEFAULT 2.5,
    interval_days INTEGER DEFAULT 0,
    repetitions INTEGER DEFAULT 0,
    next_review TIMESTAMP,
    mastery_level TEXT,
    FOREIGN KEY (item_id) REFERENCES items(id)
);

-- Review history
CREATE TABLE reviews (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    item_id TEXT NOT NULL,
    reviewed_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    quality INTEGER,
    response_time_ms INTEGER,
    was_correct BOOLEAN,
    FOREIGN KEY (item_id) REFERENCES items(id)
);

-- User statistics
CREATE TABLE user_stats (
    user_id TEXT PRIMARY KEY,
    total_reviews INTEGER DEFAULT 0,
    correct_reviews INTEGER DEFAULT 0,
    current_streak INTEGER DEFAULT 0,
    longest_streak INTEGER DEFAULT 0,
    total_study_time_ms INTEGER DEFAULT 0,
    items_mastered INTEGER DEFAULT 0,
    last_session TIMESTAMP
);

-- Indexes for performance
CREATE INDEX idx_next_review ON card_states(next_review);
CREATE INDEX idx_reviews_item ON reviews(item_id);
CREATE INDEX idx_reviews_date ON reviews(reviewed_at);
```

---

## 6. Best Practices and Recommendations

### 6.1 Data Persistence
- **Use SQLite for complex queries and relationships**
- **Use JSON for simple config and portable data**
- **Implement regular backups and data export**
- **Consider data migration strategies early**

### 6.2 Spaced Repetition
- **Start with SM-2 for simplicity**
- **Consider FSRS for advanced applications**
- **Allow users to adjust algorithm parameters**
- **Track algorithm effectiveness with A/B testing**

### 6.3 Statistics Tracking
- **Aggregate data periodically to reduce database load**
- **Use appropriate metric types (counter/gauge/histogram)**
- **Store raw events for future analysis**
- **Calculate derived metrics on-demand**

### 6.4 Visualization
- **Use Ratatui for terminal-based applications**
- **Use Plotters for detailed charts and graphs**
- **Update visualizations reactively for real-time feedback**
- **Provide multiple view options (summary, detailed, trends)**

### 6.5 Performance Considerations
- **Batch database operations when possible**
- **Use prepared statements for repeated queries**
- **Cache frequently accessed data**
- **Profile and optimize hot paths**

---

## 7. Additional Resources

### Crates
- `rusqlite` - SQLite database
- `serde` + `serde_json` - Serialization
- `spaced-repetition` - SRS algorithm
- `metrics` - Metrics collection
- `ratatui` - Terminal UI
- `plotters` - Charting library
- `chrono` - Date/time handling

### Reference Projects
- **Anki** - Open-source flashcard app (rslib in Rust)
- **Terminal-based flashcards** - github.com/mikeroit/Flashcards
- **Rust flashcards** - github.com/Krahuschluk/rust-flashcards

### Learning Resources
- SuperMemo SM-2 algorithm documentation
- FSRS algorithm research papers
- Learning analytics best practices
- Mastery learning pedagogical research

---

## 8. Implementation Checklist

- [ ] Choose persistence layer (SQLite vs JSON vs hybrid)
- [ ] Design database schema for learning data
- [ ] Implement spaced repetition algorithm (SM-2/FSRS)
- [ ] Create data structures for cards and reviews
- [ ] Set up metrics collection system
- [ ] Implement mastery level calculation
- [ ] Build statistics aggregation functions
- [ ] Create progress visualization components
- [ ] Add data export/import functionality
- [ ] Implement backup and recovery system
- [ ] Write tests for critical algorithms
- [ ] Profile and optimize performance
- [ ] Document API and data formats
