# Tasks
Implementation objectives for Japanese Learning App

## Task 2: Project Initialization
Initialize Rust project with iced framework
- Set up Cargo.toml with core dependencies (iced, redb, serde, bincode, chrono, directories, phf, ron)
- Create modular src/ directory structure (state, ui, models, persistence, scheduler, data)
- Configure release build profile with optimizations

## Task 3: Character Data System
Implement character data loading and management (hiragana first)
- Define CharData structure with unicode, romaji, stroke data, and medians
- Create RON format data files for hiragana characters
- Implement phf static map or lazy-loaded HashMap lookup
- Extract and parse SVG stroke data from kana-svg-data/animCJK

## Task 4: Japanese Font Rendering
Set up proper Japanese text display
- Embed Noto Sans JP or Source Han Sans font in binary
- Configure cosmic-text with Shaping::Advanced for proper rendering
- Create CharacterCard custom widget for flashcard display
- Test Unicode rendering with various hiragana characters

## Task 5: Simple Flashcard UI
Build basic flashcard learning interface
- Implement main menu screen with navigation
- Create flashcard view with character display
- Add answer input and validation
- Implement basic screen transitions

## Task 6: SM-2 Spaced Repetition Algorithm
Implement core SRS scheduling
- Create SM2Card structure with ease_factor, interval_days, repetitions
- Implement review() method with quality-based calculations (0-5 scale)
- Build ReviewQueue with new/learning/review prioritization
- Add next review date scheduling logic

## Task 7: Progress Persistence Layer
Set up local database storage with redb
- Initialize database in platform-specific data directory
- Create tables for cards, reviews, and settings
- Implement save_card_progress() and load_card_progress()
- Add review logging with timestamps

## Task 8: Basic Statistics Tracking
Implement progress metrics
- Create Statistics structure tracking reviews, accuracy, study time
- Add daily stats aggregation
- Implement streak calculation (current and longest)
- Track cards by state (new/learning/mastered)

## Task 9: Learning Session Flow
Create complete learning experience
- Initialize session with ReviewQueue
- Handle card presentation and answer submission
- Update SM-2 algorithm based on quality rating
- Save progress to database after each review
- Display session summary on completion

## Task 10: Stroke Order Animation System
Build GPU-accelerated stroke animations using Canvas
- Create StrokeOrderAnimation with sequential stroke playback
- Parse SVG paths to PathSegments with length calculations
- Implement partial path rendering for animation progress
- Add timing controls (500-800ms per stroke, 200-300ms pause)

## Task 11: Handwriting Canvas
Create drawing input system
- Implement HandwritingCanvas with mouse/touch event handling
- Capture stroke points with distance-based smoothing
- Draw strokes with proper line caps and joins
- Add clear canvas and stroke management

## Task 12: Visual Feedback System
Build educational feedback animations
- Create FeedbackAnimation for correct/incorrect/partial responses
- Implement color-coded feedback (green/red/yellow)
- Add shake animation for errors using easing functions
- Show immediate visual response (200-500ms)

## Task 13: Katakana Support
Extend character set to katakana
- Add katakana RON data file with 91 characters
- Update character lookup to support both sets
- Add script selection in UI
- Test rendering and animations with katakana

## Task 14: Enhanced UI & Navigation
Improve user interface
- Create character list view with grid layout
- Add practice mode selection screen
- Implement statistics dashboard view
- Build navigation menu with screen routing

## Task 15: Achievement System
Add gamification elements
- Define achievement types (first_character, hiragana_complete, streak_7, etc.)
- Implement unlock logic based on progress
- Create achievement display UI
- Add notification animations for unlocks

## Task 16: Character Recognition Integration
Integrate handwriting recognition
- Research and integrate hanzi_lookup or alternative library
- Implement stroke normalization (0-1 coordinate range)
- Return top N candidates with confidence scores
- Validate recognition against expected character

## Task 17: Progressive Practice Modes
Build multi-stage learning system
- Implement WatchAnimation mode (passive learning)
- Create TraceWithGuide mode (ghost character overlay + stroke numbers)
- Add TraceWithoutGuide mode (stroke number hints only)
- Build FreeDrawing mode (full recall, no assistance)

## Task 18: Stroke Order Validation
Validate drawing against correct stroke order
- Compare drawn strokes to reference stroke sequence
- Calculate stroke direction and position accuracy
- Provide corrective feedback for errors
- Allow mode progression based on accuracy

## Task 19: Advanced Statistics & Graphs
Enhance progress visualization
- Implement time-series data structures (daily/weekly stats)
- Create progress charts using Canvas
- Add accuracy trends and review heatmap
- Build circular progress indicators

## Task 20: FSRS Algorithm (Optional)
Implement advanced spaced repetition
- Create FSRSCard with difficulty, stability, retrievability
- Implement FSRS scheduling formulas
- Add algorithm selection in settings
- Migrate SM-2 data to FSRS format

## Task 21: Export/Import Functionality
Add data portability
- Implement progress export to JSON format
- Create import functionality with validation
- Add backup/restore features
- Support cross-device synchronization format

## Task 22: Accessibility Improvements
Ensure WCAG compliance
- Verify color contrast ratios (4.5:1 minimum)
- Add keyboard navigation support
- Implement touch-friendly targets (48x48px minimum)
- Test with screen readers (if applicable)

## Task 23: Sound Effects
Add audio feedback
- Integrate audio playback library
- Add success/error sound effects
- Implement stroke drawing sounds (optional)
- Create volume controls in settings

## Task 24: Themes & Customization
Add visual customization
- Create light/dark theme support
- Implement custom color schemes
- Add font size settings
- Build preferences screen

## Task 25: Performance Optimization
Optimize rendering and data access
- Implement Canvas geometry caching for static elements
- Add batch database operations
- Use Arc<T> for shared character data
- Profile and optimize animation frame rates

## Task 26: Testing Suite
Create comprehensive test coverage
- Write unit tests for SM-2 and FSRS algorithms
- Test ReviewQueue priority and state management
- Add integration tests for learning sessions
- Test database persistence and migration
- Validate stroke path parsing and rendering

## Task 27: Documentation
Write user and developer documentation
- Create user guide with screenshots
- Document architecture and code structure
- Write API documentation for key modules
- Add contributing guidelines

## Task 28: Deployment & Distribution
Prepare cross-platform releases
- Configure optimized release builds (LTO, strip)
- Create Linux AppImage package
- Build macOS application bundle
- Generate Windows installer (WiX/NSIS)
- Set up CI/CD pipeline for automated builds
