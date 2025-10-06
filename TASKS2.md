# Japanese Kana Learning Application - Implementation Tasks

## Phase 1: MVP (2-4 weeks)

### 1.1 Project Setup
- [ ] Initialize Rust project with iced framework
- [ ] Configure Cargo.toml with required dependencies:
  - iced (v0.13 with canvas, tokio, advanced features)
  - redb (v2.0)
  - serde, bincode
  - chrono, directories
  - phf, ron
- [ ] Set up project structure (main.rs, state/, ui/, models/, persistence/, scheduler/, data/)
- [ ] Configure release profile for optimization

### 1.2 Character Data Management
- [ ] Download/integrate kana-svg-data or animCJK datasets
- [ ] Create RON data files for hiragana characters (data/hiragana.ron)
- [ ] Implement CharData structure with unicode, romaji, stroke paths, medians
- [ ] Set up compile-time static map using phf for efficient lookups
- [ ] Add Japanese font (Noto Sans JP or Source Han Sans) to assets/
- [ ] Implement font loading at application startup

### 1.3 Basic Text Rendering
- [ ] Load Japanese font using cosmic-text
- [ ] Create CharacterCard widget for flashcard display
- [ ] Configure proper shaping (Shaping::Advanced) for Japanese text
- [ ] Test rendering of all hiragana characters

### 1.4 Spaced Repetition System (SM-2)
- [ ] Implement SM2Card structure with ease_factor, interval_days, repetitions
- [ ] Implement SM-2 algorithm review function with quality ratings (0-5)
- [ ] Create ReviewQueue with new/learning/review card prioritization
- [ ] Implement queue management logic (get_next_card)
- [ ] Set daily limits for new cards and reviews

### 1.5 Progress Persistence
- [ ] Set up redb database with three tables: CARDS_TABLE, REVIEWS_TABLE, SETTINGS_TABLE
- [ ] Implement AppDatabase with save/load methods
- [ ] Configure platform-specific data directories using directories crate
- [ ] Implement CardProgress serialization/deserialization
- [ ] Create ReviewLog structure and persistence

### 1.6 Basic UI & Navigation
- [ ] Implement AppState with screen management
- [ ] Create Screen enum (MainMenu, Learning, Statistics)
- [ ] Design Message architecture for navigation and learning events
- [ ] Implement main menu view
- [ ] Create flashcard learning view
- [ ] Add basic navigation controls

### 1.7 Learning Session
- [ ] Implement LearningSession state management
- [ ] Create flashcard display with character, romaji, answer reveal
- [ ] Add quality rating buttons (0-5)
- [ ] Implement answer submission and card updates
- [ ] Add session progress tracking (cards completed, accuracy)
- [ ] Create session end summary screen

### 1.8 Basic Statistics
- [ ] Implement Statistics structure with accuracy, study time, streaks
- [ ] Create DayStats for daily progress tracking
- [ ] Add statistics update logic after each review
- [ ] Build basic statistics view showing:
  - Total reviews
  - Accuracy rate
  - Current streak
  - Cards by status (new/learning/mastered)

## Phase 2: Core Features (3-5 weeks)

### 2.1 Stroke Order Animation System
- [ ] Implement StrokeOrderAnimation structure with animation state
- [ ] Create StrokePath with segments and length calculations
- [ ] Parse SVG path data from character data
- [ ] Implement partial path rendering for animation effect
- [ ] Configure animation timing (700ms per stroke, 250ms pause)
- [ ] Add animation controls (play, pause, restart)
- [ ] Integrate lilt/Animation API with Canvas widget

### 2.2 Drawing Canvas
- [ ] Implement HandwritingCanvas with DrawingState
- [ ] Add mouse/touch event handling (ButtonPressed, CursorMoved, ButtonReleased)
- [ ] Create Stroke capture with point collection and noise reduction
- [ ] Implement stroke rendering with configurable colors and widths
- [ ] Add canvas clear functionality
- [ ] Create drawing mode toggle (Freehand vs StrokeOrder)

### 2.3 Visual Feedback System
- [ ] Create FeedbackAnimation structure (Correct/Incorrect/Partial)
- [ ] Implement color-coded feedback (green/red/yellow)
- [ ] Add shake animation for errors
- [ ] Create checkmark/X icon animations
- [ ] Implement flash effects for correct answers
- [ ] Add sound effects (optional)

### 2.4 Katakana Support
- [ ] Create katakana.ron data file
- [ ] Extend character lookup to include katakana
- [ ] Add katakana selection in UI
- [ ] Update ReviewQueue to support multiple character sets
- [ ] Test all katakana characters render correctly

### 2.5 Enhanced Navigation
- [ ] Create character list view with grid layout
- [ ] Add filtering by character set (hiragana/katakana)
- [ ] Implement practice mode selection
- [ ] Create settings screen
- [ ] Add help/tutorial screen

### 2.6 Achievement System
- [ ] Define Achievement structure with unlock conditions
- [ ] Create achievement definitions (first character, completion, streaks)
- [ ] Implement achievement checking after reviews
- [ ] Design achievement notification UI
- [ ] Add achievements view to statistics screen

## Phase 3: Advanced Features (4-6 weeks)

### 3.1 Character Recognition
- [ ] Research/integrate hanzi_lookup or alternative recognition library
- [ ] Implement stroke data normalization (0-1 coordinate range)
- [ ] Create recognition workflow (capture → normalize → recognize → validate)
- [ ] Add confidence scoring for recognition results
- [ ] Handle top N candidates display
- [ ] Implement fallback for unrecognized input

### 3.2 Progressive Practice Modes
- [ ] Implement PracticeMode enum (WatchAnimation, TraceWithGuide, TraceWithoutGuide, FreeDrawing)
- [ ] Create mode progression logic
- [ ] Build TraceWithGuide view (ghosted character overlay + stroke numbers)
- [ ] Build TraceWithoutGuide view (stroke numbers only)
- [ ] Build FreeDrawing view (no assistance)
- [ ] Add mode transition animations

### 3.3 Stroke Order Validation
- [ ] Implement stroke sequence validation logic
- [ ] Compare user strokes against expected medians
- [ ] Calculate stroke accuracy scores
- [ ] Provide specific feedback on incorrect strokes
- [ ] Highlight problematic strokes in red

### 3.4 Advanced Statistics & Graphs
- [ ] Add weekly/monthly aggregation (WeekStats)
- [ ] Create line chart for accuracy over time
- [ ] Build heatmap for study consistency
- [ ] Add character-specific statistics
- [ ] Implement detailed review history view
- [ ] Create exportable reports (CSV/JSON)

### 3.5 FSRS Algorithm (Optional)
- [ ] Implement FSRSCard structure with difficulty, stability, retrievability
- [ ] Create FSRS review algorithm
- [ ] Build parameter optimization system
- [ ] Add algorithm selection in settings
- [ ] Migrate SM-2 data to FSRS format
- [ ] Compare performance metrics between algorithms

### 3.6 Data Export/Import
- [ ] Implement progress export to JSON/CSV
- [ ] Create backup functionality
- [ ] Add import from backup files
- [ ] Support cross-device sync preparation
- [ ] Validate imported data integrity

## Phase 4: Polish (2-3 weeks)

### 4.1 Accessibility Improvements
- [ ] Ensure WCAG AA color contrast (4.5:1 for text)
- [ ] Add high-contrast theme option
- [ ] Implement keyboard navigation for all screens
- [ ] Add screen reader support labels
- [ ] Ensure touch targets are 48x48 pixels minimum
- [ ] Test with accessibility tools

### 4.2 UI/UX Refinements
- [ ] Design and implement consistent color palette
- [ ] Add smooth transitions between screens
- [ ] Implement progress indicators (circular, linear)
- [ ] Polish button styles and hover states
- [ ] Add loading states for async operations
- [ ] Create empty states for lists

### 4.3 Performance Optimization
- [ ] Implement canvas caching for static elements
- [ ] Optimize database batch operations
- [ ] Add lazy loading for character data
- [ ] Profile and optimize animation rendering
- [ ] Reduce binary size (strip symbols)
- [ ] Test memory usage under load

### 4.4 Sound Effects
- [ ] Source/create sound files (correct, incorrect, completion)
- [ ] Integrate audio playback library
- [ ] Add sound toggle in settings
- [ ] Implement volume control

### 4.5 Themes & Customization
- [ ] Create light/dark theme support
- [ ] Add custom color scheme option
- [ ] Allow font size adjustment
- [ ] Implement animation speed control
- [ ] Add daily goal customization

### 4.6 Testing & Quality Assurance
- [ ] Write unit tests for SM-2 algorithm
- [ ] Test ReviewQueue priority logic
- [ ] Create integration tests for learning sessions
- [ ] Test database persistence and recovery
- [ ] Perform cross-platform testing (Linux, macOS, Windows)
- [ ] Load testing with large review histories
- [ ] Fix identified bugs

### 4.7 Documentation
- [ ] Write user guide
- [ ] Create developer documentation
- [ ] Document build/deployment process
- [ ] Add inline code documentation
- [ ] Create architecture diagram
- [ ] Write contribution guidelines

### 4.8 Deployment
- [ ] Configure release builds (opt-level=3, lto=true)
- [ ] Create Linux AppImage
- [ ] Build macOS application bundle
- [ ] Create Windows installer
- [ ] Set up CI/CD pipeline
- [ ] Publish releases on GitHub

## Future Enhancements (Post-MVP)

### Advanced Learning Features
- [ ] Add vocabulary integration (words using learned kana)
- [ ] Implement listening practice mode
- [ ] Create typing practice mode
- [ ] Add mnemonics/memory aids
- [ ] Support custom flashcard decks

### Social & Gamification
- [ ] Add leaderboards
- [ ] Implement friend challenges
- [ ] Create badge/trophy system
- [ ] Add daily/weekly quests

### Content Expansion
- [ ] Add kanji learning support
- [ ] Include stroke order for complex characters
- [ ] Support hiragana/katakana combinations (dakuten, handakuten)
- [ ] Add rare character support

### Technical Improvements
- [ ] Implement cloud sync
- [ ] Add mobile companion app
- [ ] Create web version (WASM)
- [ ] Support offline-first architecture
- [ ] Add telemetry/analytics (opt-in)

---

## Development Priorities

**Critical Path (Minimum Viable Product):**
1. Character rendering → Flashcards → SM-2 → Persistence → Basic stats
2. This provides a functional learning tool immediately

**High Priority (Core Features):**
3. Stroke animations → Drawing canvas → Visual feedback
4. These significantly enhance learning effectiveness

**Medium Priority (Advanced Features):**
5. Recognition → Progressive modes → Advanced stats
6. These improve user experience but aren't blocking

**Low Priority (Polish):**
7. Themes → Sound → Extra gamification
8. These add polish but can be deferred

## Risk Mitigation

- **Character Recognition Complexity**: Start with simpler validation, add recognition later
- **Performance Issues**: Profile early, cache aggressively, optimize rendering
- **Data Loss**: Implement auto-save, backups, and data validation
- **Cross-Platform Compatibility**: Test on all platforms regularly
- **Scope Creep**: Stick to MVP first, add features incrementally
