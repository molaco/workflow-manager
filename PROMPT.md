new workflow:

task:
  id: 5
  name: "Handwriting Canvas and Input System"

context:
  description: |
    The Handwriting Canvas and Input System provides an interactive drawing surface
    that captures user handwriting input through mouse or touch events. This component
    is essential for kinesthetic learning in the Japanese character learning application,
    enabling users to practice character writing with immediate visual feedback.

    The system implements a complete stroke recording pipeline: capturing raw input events,
    filtering noise through distance thresholding, rendering smooth anti-aliased strokes,
    and exporting normalized stroke data suitable for character recognition validation.

    Architecturally, this component bridges the UI interaction layer and the recognition
    system. It transforms raw pointer events into structured stroke sequences with temporal
    information, enabling downstream components to analyze writing patterns and validate
    character recognition.

    The implementation leverages iced's canvas::Program trait to provide a responsive,
    cross-platform drawing experience that works seamlessly on both desktop (mouse) and
    touch-enabled devices. The stroke recording system maintains a clean separation between
    current drawing state and completed stroke history, supporting undo operations and
    multiple stroke sequences essential for complex character writing.

  key_points:
    - "Implements canvas::Program trait for event-driven interactive drawing with proper state machine transitions"
    - "Applies 2-pixel distance threshold to reduce input noise while maintaining stroke fidelity"
    - "Renders strokes with anti-aliasing and round caps/joins for natural handwriting appearance"
    - "Records timestamps with each point to enable future velocity analysis and recognition enhancement"
    - "Normalizes coordinates to 0-1 range for device-independent character recognition"
    - "Maintains clear separation between current active stroke and completed stroke history"
    - "Provides essential editing operations (clear, undo) for practice workflow"
    - "Exports stroke data in format compatible with recognition system (Task 9)"

files:
  - path: "src/ui/handwriting_canvas.rs"
    description: "Implements HandwritingCanvas widget with canvas::Program trait for interactive drawing, handling mouse/touch events and rendering strokes"
  - path: "src/ui/drawing_state.rs"
    description: "Maintains DrawingState struct with stroke history, current active stroke, and state machine for drawing lifecycle"
  - path: "src/ui/canvas_message.rs"
    description: "Defines CanvasMessage enum for canvas interaction events (StartStroke, AddPoint, EndStroke, Clear, Undo)"
  - path: "src/ui/canvas_renderer.rs"
    description: "Provides rendering helper functions for drawing strokes with anti-aliasing and proper styling"
  - path: "src/ui/mod.rs"
    description: "UI module root declaring and exposing handwriting_canvas, drawing_state, canvas_message, and canvas_renderer submodules"
  - path: "src/types/stroke.rs"
    description: "Defines Stroke and Point data structures with normalization and distance calculation methods"
  - path: "src/types/mod.rs"
    description: "Types module root declaring stroke module for stroke-related data structures"
  - path: "src/lib.rs"
    description: "Library root declaring and exposing ui and types modules for the application"
  - path: "Cargo.toml"
    description: "Project manifest - add iced dependency with canvas feature for interactive drawing support"

functions:
  - file: "src/ui/handwriting_canvas.rs"
    items:
      - type: "struct"
        name: "HandwritingCanvas"
        description: "Canvas widget that captures mouse/touch input and renders handwriting strokes in real-time, implementing canvas::Program trait"
        invariants: "State is never null; strokes list is ordered chronologically"

      - type: "method"
        name: "HandwritingCanvas::new"
        description: "Creates a new HandwritingCanvas instance with empty state"
        postconditions: "Returns initialized canvas with no strokes"

      - type: "method"
        name: "HandwritingCanvas::clear"
        description: "Removes all strokes from the canvas"
        postconditions: "All stroke data is cleared"

      - type: "method"
        name: "HandwritingCanvas::undo"
        description: "Removes the last completed stroke"
        postconditions: "Most recent stroke is removed if strokes exist"

      - type: "method"
        name: "HandwritingCanvas::get_strokes"
        description: "Returns the current stroke data as a reference"
        postconditions: "Returns immutable reference to stroke vector"

      - type: "method"
        name: "HandwritingCanvas::export_normalized_strokes"
        description: "Exports stroke data normalized to 0-1 coordinate range for character recognition"
        postconditions: "Returns Vec<Vec<(f32, f32)>> with normalized coordinates"

      - type: "trait_impl"
        name: "canvas::Program for HandwritingCanvas"
        description: "Implements iced canvas::Program trait for interactive drawing functionality with update, draw, and mouse_interaction methods"

      - type: "method"
        name: "canvas::Program::update"
        description: "Handles canvas messages (StartStroke, AddPoint, EndStroke, Clear, Undo) and updates drawing state"
        preconditions: "Valid Message is provided"
        postconditions: "State is updated according to message type; returns appropriate action for redraw"

      - type: "method"
        name: "canvas::Program::draw"
        description: "Renders all completed strokes and current active stroke to the canvas with anti-aliasing"
        postconditions: "All strokes are rendered with anti-aliasing and round caps/joins"

      - type: "method"
        name: "canvas::Program::mouse_interaction"
        description: "Provides cursor feedback based on canvas state (crosshair while drawing, default otherwise)"
        postconditions: "Returns appropriate cursor style"

      - type: "constant"
        name: "STROKE_WIDTH"
        description: "Default width (3.0 pixels) for rendered strokes"

      - type: "constant"
        name: "DRAWING_COLOR"
        description: "Color used for the current active stroke being drawn (e.g., blue or gray)"

      - type: "constant"
        name: "COMPLETED_COLOR"
        description: "Color used for completed strokes (e.g., black)"

  - file: "src/ui/drawing_state.rs"
    items:
      - type: "struct"
        name: "DrawingState"
        description: "Maintains the complete drawing state including stroke history, current active stroke, and drawing flag"
        invariants: "current_stroke is Some only when is_drawing is true"

      - type: "method"
        name: "DrawingState::new"
        description: "Creates a new empty drawing state"
        postconditions: "Returns initialized state with empty stroke list, is_drawing false, current_stroke None"

      - type: "method"
        name: "DrawingState::start_stroke"
        description: "Begins a new stroke at the given position with timestamp"
        preconditions: "No stroke is currently active"
        postconditions: "current_stroke is Some with initial point; is_drawing is true"

      - type: "method"
        name: "DrawingState::add_point"
        description: "Adds a point to the current stroke if distance threshold is met (2px)"
        preconditions: "A stroke is currently active (is_drawing is true)"
        postconditions: "Point is added if distance from last point exceeds threshold (2px)"

      - type: "method"
        name: "DrawingState::end_stroke"
        description: "Completes the current stroke and adds it to the stroke history"
        preconditions: "A stroke is currently active"
        postconditions: "current_stroke is None; completed stroke is added to strokes list; is_drawing is false"

      - type: "method"
        name: "DrawingState::clear_all"
        description: "Removes all strokes and resets state"
        postconditions: "strokes is empty; current_stroke is None; is_drawing is false"

      - type: "method"
        name: "DrawingState::undo_last"
        description: "Removes the most recent completed stroke"
        postconditions: "Last element removed from strokes if list is non-empty"

      - type: "method"
        name: "DrawingState::get_completed_strokes"
        description: "Returns reference to completed strokes list"
        postconditions: "Returns immutable reference to Vec<Stroke>"

      - type: "method"
        name: "DrawingState::get_current_stroke"
        description: "Returns reference to current active stroke if any"
        postconditions: "Returns Option<&Stroke>"

      - type: "constant"
        name: "DISTANCE_THRESHOLD"
        description: "Minimum distance (2.0 pixels) between consecutive points to reduce noise"

  - file: "src/types/stroke.rs"
    items:
      - type: "struct"
        name: "Stroke"
        description: "Represents a single continuous stroke as a sequence of points with timestamps"
        invariants: "Points vector is never empty for a completed stroke; timestamps are monotonically increasing"

      - type: "method"
        name: "Stroke::new"
        description: "Creates a new stroke starting at the given point with timestamp"
        postconditions: "Returns stroke with single initial point"

      - type: "method"
        name: "Stroke::add_point"
        description: "Appends a point to the stroke"
        postconditions: "Point is added to points vector"

      - type: "method"
        name: "Stroke::to_path"
        description: "Converts stroke points into a lyon Path for rendering"
        preconditions: "Stroke has at least one point"
        postconditions: "Returns Path with line_to segments connecting all points with round caps and joins"

      - type: "method"
        name: "Stroke::normalize"
        description: "Normalizes stroke coordinates to 0-1 range based on canvas bounds"
        preconditions: "canvas_width and canvas_height are positive"
        postconditions: "Returns vector of normalized (f32, f32) tuples where all values are 0.0 to 1.0"

      - type: "method"
        name: "Stroke::len"
        description: "Returns the number of points in the stroke"
        postconditions: "Returns usize count of points"

      - type: "method"
        name: "Stroke::is_empty"
        description: "Checks if stroke has no points"
        postconditions: "Returns true if points vector is empty"

      - type: "struct"
        name: "Point"
        description: "Represents a single point in a stroke with x, y coordinates and timestamp"
        invariants: "Coordinates are within canvas bounds when recorded"

      - type: "method"
        name: "Point::new"
        description: "Creates a new point with coordinates and timestamp"
        postconditions: "Returns initialized Point with given x, y, and timestamp"

      - type: "method"
        name: "Point::distance_to"
        description: "Calculates Euclidean distance to another point"
        postconditions: "Returns non-negative f32 distance value"

  - file: "src/ui/canvas_message.rs"
    items:
      - type: "enum"
        name: "CanvasMessage"
        description: "Messages for canvas interaction events"

      - type: "enum_variant"
        name: "CanvasMessage::StartStroke"
        description: "Initiates a new stroke at the given position (contains Point)"
        preconditions: "No stroke is currently active"
        postconditions: "A new stroke is started in DrawingState"

      - type: "enum_variant"
        name: "CanvasMessage::AddPoint"
        description: "Adds a point to the current active stroke (contains Point)"
        preconditions: "A stroke is currently active (is_drawing is true)"
        postconditions: "Point is added to current stroke if distance threshold is met"

      - type: "enum_variant"
        name: "CanvasMessage::EndStroke"
        description: "Completes the current stroke and adds it to stroke history"
        preconditions: "A stroke is currently active"
        postconditions: "Current stroke is moved to completed strokes list"

      - type: "enum_variant"
        name: "CanvasMessage::Clear"
        description: "Removes all strokes from the canvas"
        postconditions: "All strokes are removed; canvas is blank"

      - type: "enum_variant"
        name: "CanvasMessage::Undo"
        description: "Removes the most recently completed stroke"
        postconditions: "Last stroke is removed from strokes list if any exist"

  - file: "src/ui/canvas_renderer.rs"
    items:
      - type: "function"
        name: "render_stroke"
        description: "Helper function to render a stroke with specified style (color, width) to the canvas frame"
        preconditions: "Stroke has at least 2 points for visible rendering; frame is valid"
        postconditions: "Stroke is drawn to frame with anti-aliasing, round caps and joins"

      - type: "function"
        name: "render_completed_strokes"
        description: "Renders all completed strokes to the canvas with completed stroke styling"
        postconditions: "All completed strokes are rendered in COMPLETED_COLOR"

      - type: "function"
        name: "render_current_stroke"
        description: "Renders the current active stroke being drawn with active stroke styling"
        postconditions: "Current stroke is rendered in DRAWING_COLOR if exists"

  - file: "src/ui/mod.rs"
    items:
      - type: "module_declaration"
        name: "handwriting_canvas"
        description: "Module containing HandwritingCanvas widget implementation"

      - type: "module_declaration"
        name: "drawing_state"
        description: "Module containing DrawingState implementation"

      - type: "module_declaration"
        name: "canvas_message"
        description: "Module containing CanvasMessage enum"

      - type: "module_declaration"
        name: "canvas_renderer"
        description: "Module containing rendering helper functions"

  - file: "src/types/mod.rs"
    items:
      - type: "module_declaration"
        name: "stroke"
        description: "Module containing Stroke and Point data structures"

formal_verification:
  needed: false
  level: "None"
  explanation: |
    Formal verification is not required for the handwriting canvas input system for the following reasons:

    1. Non-Critical UI Component: The handwriting canvas is an interactive UI component for capturing user drawing input. Errors in stroke rendering or input handling are immediately visible to users and do not pose safety, security, or data integrity risks. Users can simply redraw if input is incorrect.

    2. Simple State Machine: The system implements a straightforward three-state machine (idle → drawing → stroke complete) with deterministic transitions triggered by mouse/touch events. The state transitions are simple enough that standard unit and integration tests provide adequate verification of correctness.

    3. Adequate Testing Coverage: The identified critical properties (stroke continuity, point sampling rate, stroke completion callbacks, stroke sequence ordering) can be effectively verified through:
       - Unit tests for stroke data structures and point normalization
       - Integration tests with simulated mouse events to verify event handling
       - Property-based tests for distance threshold and coordinate normalization invariants

    4. Low-Risk Failure Mode: The canvas operates on ephemeral local drawing data that can be easily regenerated. Unlike systems handling persistent data, financial transactions, or safety-critical operations, incorrect behavior only affects the current drawing session with no lasting consequences.

    5. Simple Arithmetic Operations: The distance threshold filtering (2px minimum) and coordinate normalization (0-1 range) are straightforward arithmetic operations that can be thoroughly validated through property-based testing without requiring formal proofs of correctness.

    6. Cost-Benefit Analysis: The complexity and cost of formal verification would far outweigh the benefits for this UI component. The testing strategy already outlined (unit + integration + property-based) provides sufficient confidence at a fraction of the cost.

    The task specification already correctly identifies formal_verification: false with integration_testing: true as the appropriate verification approach. Standard testing practices are both necessary and sufficient for this component.

tests:
  strategy:
    approach: "mixed (unit + integration + property-based)"
    rationale:
      - "Unit tests verify stroke data structures, point addition, and normalization logic in isolation"
      - "Integration tests validate complete drawing flow with simulated mouse events and state transitions"
      - "Property-based tests ensure distance threshold and normalization work correctly across all input ranges"
      - "Canvas widget implements canvas::Program trait requiring integration testing for event handling state machine"
      - "Critical properties like stroke continuity and coordinate bounds need systematic verification"
      - "Mixed approach provides comprehensive coverage from low-level data structures to high-level user interactions"

  implementation:
    file: "src/ui/handwriting_canvas.rs"
    location: "in existing test module"
    code: |
      #[cfg(test)]
      mod tests {
          use super::*;
          use iced::Point as IcedPoint;
          use std::time::Duration;

          // Helper function to calculate distance between two points
          fn calculate_distance(p1: IcedPoint, p2: IcedPoint) -> f32 {
              let dx = p2.x - p1.x;
              let dy = p2.y - p1.y;
              (dx * dx + dy * dy).sqrt()
          }

          // Helper function to determine if point should be added based on threshold
          fn should_add_point(last_point: IcedPoint, new_point: IcedPoint, threshold: f32) -> bool {
              calculate_distance(last_point, new_point) >= threshold
          }

          // Helper function to simulate mouse drag across canvas
          fn simulate_mouse_drag(state: &mut DrawingState, start: IcedPoint, end: IcedPoint, steps: usize) {
              state.start_stroke(start);

              for i in 1..=steps {
                  let t = i as f32 / steps as f32;
                  let x = start.x + (end.x - start.x) * t;
                  let y = start.y + (end.y - start.y) * t;
                  state.add_point(IcedPoint::new(x, y));
              }

              state.complete_stroke();
          }

          // ========== Unit Tests: Stroke Data Structure ==========

          #[test]
          fn test_stroke_creation_empty() {
              // Verify that a new stroke is initialized with empty point and timestamp vectors
              let stroke = Stroke::new();
              assert!(stroke.points.is_empty());
              assert!(stroke.timestamps.is_empty());
          }

          #[test]
          fn test_stroke_add_point() {
              // Test adding a single point to a stroke updates both points and timestamps
              let mut stroke = Stroke::new();
              let point = IcedPoint::new(10.0, 20.0);
              stroke.add_point(point);

              assert_eq!(stroke.points.len(), 1);
              assert_eq!(stroke.timestamps.len(), 1);
              assert_eq!(stroke.points[0], point);
          }

          #[test]
          fn test_stroke_add_multiple_points() {
              // Verify that multiple points are stored in sequence
              let mut stroke = Stroke::new();
              let points = vec![
                  IcedPoint::new(10.0, 10.0),
                  IcedPoint::new(20.0, 20.0),
                  IcedPoint::new(30.0, 30.0),
              ];

              for point in points.iter() {
                  stroke.add_point(*point);
              }

              assert_eq!(stroke.points.len(), 3);
              assert_eq!(stroke.timestamps.len(), 3);

              for (i, point) in points.iter().enumerate() {
                  assert_eq!(stroke.points[i], *point);
              }
          }

          #[test]
          fn test_stroke_normalization() {
              // Test coordinate normalization to 0-1 range based on canvas dimensions
              let mut stroke = Stroke::new();
              stroke.add_point(IcedPoint::new(0.0, 0.0));
              stroke.add_point(IcedPoint::new(100.0, 50.0));
              stroke.add_point(IcedPoint::new(200.0, 200.0));

              let normalized = stroke.normalize(200.0, 200.0);

              assert_eq!(normalized.len(), 3);
              assert_eq!(normalized[0].x, 0.0);
              assert_eq!(normalized[0].y, 0.0);
              assert_eq!(normalized[1].x, 0.5);
              assert_eq!(normalized[1].y, 0.25);
              assert_eq!(normalized[2].x, 1.0);
              assert_eq!(normalized[2].y, 1.0);
          }

          #[test]
          fn test_normalization_preserves_aspect_ratio() {
              // Ensure normalization maintains relative positioning correctly
              let mut stroke = Stroke::new();
              stroke.add_point(IcedPoint::new(0.0, 0.0));
              stroke.add_point(IcedPoint::new(100.0, 200.0));

              let normalized = stroke.normalize(200.0, 200.0);

              assert!((normalized[1].x - 0.5).abs() < 0.001);
              assert!((normalized[1].y - 1.0).abs() < 0.001);
          }

          #[test]
          fn test_canvas_bounds_normalization() {
              // Test corner points normalize to exact boundary values
              let mut stroke = Stroke::new();
              stroke.add_point(IcedPoint::new(0.0, 0.0));
              stroke.add_point(IcedPoint::new(200.0, 0.0));
              stroke.add_point(IcedPoint::new(200.0, 200.0));
              stroke.add_point(IcedPoint::new(0.0, 200.0));

              let normalized = stroke.normalize(200.0, 200.0);

              assert_eq!(normalized[0], IcedPoint::new(0.0, 0.0));
              assert_eq!(normalized[1], IcedPoint::new(1.0, 0.0));
              assert_eq!(normalized[2], IcedPoint::new(1.0, 1.0));
              assert_eq!(normalized[3], IcedPoint::new(0.0, 1.0));
          }

          // ========== Unit Tests: Distance Threshold ==========

          #[test]
          fn test_distance_calculation() {
              // Verify Euclidean distance calculation (3-4-5 triangle)
              let p1 = IcedPoint::new(0.0, 0.0);
              let p2 = IcedPoint::new(3.0, 4.0);
              let distance = calculate_distance(p1, p2);

              assert!((distance - 5.0).abs() < 0.001);
          }

          #[test]
          fn test_distance_threshold_filtering() {
              // Test that points within threshold are rejected, points beyond are accepted
              let point1 = IcedPoint::new(10.0, 10.0);
              let point2 = IcedPoint::new(10.5, 10.5); // Distance ~0.707px
              let point3 = IcedPoint::new(15.0, 15.0); // Distance ~7.07px

              assert!(!should_add_point(point1, point2, 2.0));
              assert!(should_add_point(point1, point3, 2.0));
          }

          #[test]
          fn test_distance_threshold_consistency() {
              // Verify that distance threshold consistently filters noise across a stroke
              let mut state = DrawingState::new();
              state.start_stroke(IcedPoint::new(10.0, 10.0));

              // Add 20 points very close together (0.1px apart)
              for i in 1..=20 {
                  let point = IcedPoint::new(10.0 + (i as f32) * 0.1, 10.0);
                  state.add_point_with_threshold(point, 2.0);
              }

              if let Some(ref stroke) = state.current_stroke {
                  // Should have far fewer than 20 points due to threshold
                  assert!(stroke.points.len() < 20);

                  // Verify consecutive points are at least ~2px apart (allowing small tolerance)
                  for i in 1..stroke.points.len() {
                      let dist = calculate_distance(stroke.points[i-1], stroke.points[i]);
                      assert!(dist >= 1.9);
                  }
              }
          }

          // ========== Unit Tests: DrawingState Management ==========

          #[test]
          fn test_drawing_state_initial() {
              // Verify DrawingState initializes to empty/idle state
              let state = DrawingState::new();

              assert!(!state.is_drawing);
              assert!(state.current_stroke.is_none());
              assert!(state.completed_strokes.is_empty());
          }

          #[test]
          fn test_drawing_state_start_stroke() {
              // Test starting a stroke transitions to drawing state with initial point
              let mut state = DrawingState::new();
              state.start_stroke(IcedPoint::new(10.0, 10.0));

              assert!(state.is_drawing);
              assert!(state.current_stroke.is_some());
              if let Some(ref stroke) = state.current_stroke {
                  assert_eq!(stroke.points.len(), 1);
              }
          }

          #[test]
          fn test_drawing_state_add_point_to_current_stroke() {
              // Verify points are added to the current active stroke
              let mut state = DrawingState::new();
              state.start_stroke(IcedPoint::new(10.0, 10.0));
              state.add_point(IcedPoint::new(15.0, 15.0));

              if let Some(ref stroke) = state.current_stroke {
                  assert_eq!(stroke.points.len(), 2);
              } else {
                  panic!("Current stroke should exist");
              }
          }

          #[test]
          fn test_drawing_state_complete_stroke() {
              // Test that completing a stroke moves it to history and resets state
              let mut state = DrawingState::new();
              state.start_stroke(IcedPoint::new(10.0, 10.0));
              state.add_point(IcedPoint::new(15.0, 15.0));
              state.complete_stroke();

              assert!(!state.is_drawing);
              assert!(state.current_stroke.is_none());
              assert_eq!(state.completed_strokes.len(), 1);
              assert_eq!(state.completed_strokes[0].points.len(), 2);
          }

          #[test]
          fn test_drawing_state_clear() {
              // Verify clear removes all strokes and resets to initial state
              let mut state = DrawingState::new();
              state.start_stroke(IcedPoint::new(10.0, 10.0));
              state.complete_stroke();
              state.start_stroke(IcedPoint::new(20.0, 20.0));
              state.complete_stroke();

              assert_eq!(state.completed_strokes.len(), 2);

              state.clear();

              assert!(state.completed_strokes.is_empty());
              assert!(!state.is_drawing);
              assert!(state.current_stroke.is_none());
          }

          #[test]
          fn test_drawing_state_undo() {
              // Test undo removes most recent stroke from history
              let mut state = DrawingState::new();
              state.start_stroke(IcedPoint::new(10.0, 10.0));
              state.complete_stroke();
              state.start_stroke(IcedPoint::new(20.0, 20.0));
              state.complete_stroke();

              assert_eq!(state.completed_strokes.len(), 2);

              state.undo();

              assert_eq!(state.completed_strokes.len(), 1);
          }

          #[test]
          fn test_undo_empty_does_nothing() {
              // Verify undo on empty state doesn't cause errors
              let mut state = DrawingState::new();
              state.undo();

              assert!(state.completed_strokes.is_empty());
          }

          // ========== Unit Tests: Data Export ==========

          #[test]
          fn test_export_stroke_data_empty() {
              // Test exporting from empty canvas returns empty vector
              let state = DrawingState::new();
              let exported = state.export_stroke_data(200.0, 200.0);

              assert!(exported.is_empty());
          }

          #[test]
          fn test_export_stroke_data_multiple_strokes() {
              // Verify export includes all strokes with normalized coordinates
              let mut state = DrawingState::new();

              state.start_stroke(IcedPoint::new(0.0, 0.0));
              state.add_point(IcedPoint::new(100.0, 100.0));
              state.complete_stroke();

              state.start_stroke(IcedPoint::new(50.0, 50.0));
              state.add_point(IcedPoint::new(150.0, 150.0));
              state.complete_stroke();

              let exported = state.export_stroke_data(200.0, 200.0);

              assert_eq!(exported.len(), 2);
              assert_eq!(exported[0].len(), 2);
              assert_eq!(exported[1].len(), 2);

              // Verify all coordinates are in [0,1] range
              assert!(exported[0][0].x >= 0.0 && exported[0][0].x <= 1.0);
              assert!(exported[0][0].y >= 0.0 && exported[0][0].y <= 1.0);
          }

          // ========== Integration Tests: Stroke Sequence ==========

          #[test]
          fn test_multiple_strokes_maintain_sequence() {
              // Verify strokes are stored in chronological order
              let mut state = DrawingState::new();

              state.start_stroke(IcedPoint::new(10.0, 10.0));
              state.complete_stroke();

              state.start_stroke(IcedPoint::new(20.0, 20.0));
              state.complete_stroke();

              state.start_stroke(IcedPoint::new(30.0, 30.0));
              state.complete_stroke();

              assert_eq!(state.completed_strokes.len(), 3);
              assert_eq!(state.completed_strokes[0].points[0].x, 10.0);
              assert_eq!(state.completed_strokes[1].points[0].x, 20.0);
              assert_eq!(state.completed_strokes[2].points[0].x, 30.0);
          }

          #[test]
          fn test_stroke_continuity_no_gaps() {
              // Verify stroke points form continuous sequence without gaps
              let mut stroke = Stroke::new();
              let points = vec![
                  IcedPoint::new(10.0, 10.0),
                  IcedPoint::new(15.0, 15.0),
                  IcedPoint::new(20.0, 20.0),
                  IcedPoint::new(25.0, 25.0),
              ];

              for point in points.iter() {
                  stroke.add_point(*point);
              }

              assert_eq!(stroke.points.len(), 4);

              // Verify each point is stored correctly in sequence
              for (i, point) in points.iter().enumerate() {
                  assert_eq!(stroke.points[i], *point);
              }
          }

          #[test]
          fn test_timestamp_recording() {
              // Verify timestamps are recorded and monotonically increasing
              let mut stroke = Stroke::new();

              stroke.add_point(IcedPoint::new(10.0, 10.0));
              std::thread::sleep(Duration::from_millis(10));
              stroke.add_point(IcedPoint::new(20.0, 20.0));

              assert_eq!(stroke.timestamps.len(), 2);

              let t1 = stroke.timestamps[0];
              let t2 = stroke.timestamps[1];
              assert!(t2 > t1);

              let duration = t2.duration_since(t1).unwrap();
              assert!(duration.as_millis() >= 10);
          }

          // ========== Integration Tests: Complete Drawing Flow ==========

          #[test]
          fn test_integration_complete_drawing_flow() {
              // Test complete flow: start -> add points -> complete -> export
              let mut state = DrawingState::new();

              simulate_mouse_drag(
                  &mut state,
                  IcedPoint::new(10.0, 10.0),
                  IcedPoint::new(100.0, 100.0),
                  10
              );

              assert_eq!(state.completed_strokes.len(), 1);
              assert!(state.completed_strokes[0].points.len() > 0);

              let exported = state.export_stroke_data(200.0, 200.0);
              assert_eq!(exported.len(), 1);
              assert!(exported[0].len() > 0);
          }

          #[test]
          fn test_integration_multiple_stroke_sequence() {
              // Test drawing multiple strokes in sequence
              let mut state = DrawingState::new();

              simulate_mouse_drag(&mut state, IcedPoint::new(10.0, 10.0), IcedPoint::new(50.0, 50.0), 5);
              simulate_mouse_drag(&mut state, IcedPoint::new(60.0, 10.0), IcedPoint::new(100.0, 50.0), 5);
              simulate_mouse_drag(&mut state, IcedPoint::new(10.0, 60.0), IcedPoint::new(50.0, 100.0), 5);

              assert_eq!(state.completed_strokes.len(), 3);

              let exported = state.export_stroke_data(200.0, 200.0);
              assert_eq!(exported.len(), 3);
          }

          #[test]
          fn test_integration_clear_during_drawing() {
              // Verify clear cancels current drawing and removes all strokes
              let mut state = DrawingState::new();

              simulate_mouse_drag(&mut state, IcedPoint::new(10.0, 10.0), IcedPoint::new(50.0, 50.0), 5);
              state.start_stroke(IcedPoint::new(60.0, 60.0));
              state.add_point(IcedPoint::new(70.0, 70.0));

              state.clear();

              assert!(state.completed_strokes.is_empty());
              assert!(!state.is_drawing);
              assert!(state.current_stroke.is_none());
          }

          #[test]
          fn test_integration_undo_sequence() {
              // Test sequential undo operations and edge case of undo on empty
              let mut state = DrawingState::new();

              simulate_mouse_drag(&mut state, IcedPoint::new(10.0, 10.0), IcedPoint::new(50.0, 50.0), 5);
              simulate_mouse_drag(&mut state, IcedPoint::new(60.0, 10.0), IcedPoint::new(100.0, 50.0), 5);
              simulate_mouse_drag(&mut state, IcedPoint::new(10.0, 60.0), IcedPoint::new(50.0, 100.0), 5);

              assert_eq!(state.completed_strokes.len(), 3);

              state.undo();
              assert_eq!(state.completed_strokes.len(), 2);

              state.undo();
              assert_eq!(state.completed_strokes.len(), 1);

              state.undo();
              assert_eq!(state.completed_strokes.len(), 0);

              // Undo on empty should not panic
              state.undo();
              assert_eq!(state.completed_strokes.len(), 0);
          }
      }

      #[cfg(test)]
      mod property_tests {
          use super::*;
          use iced::Point as IcedPoint;

          // Note: Requires proptest crate in Cargo.toml for property-based testing
          // [dev-dependencies]
          // proptest = "1.0"

          #[cfg(feature = "proptest")]
          use proptest::prelude::*;

          #[cfg(feature = "proptest")]
          proptest! {
              #[test]
              fn prop_normalization_bounds(
                  x in 0.0f32..1000.0,
                  y in 0.0f32..1000.0,
                  width in 100.0f32..1000.0,
                  height in 100.0f32..1000.0
              ) {
                  // Property: Normalized coordinates must always be in [0,1] range
                  let mut stroke = Stroke::new();
                  stroke.add_point(IcedPoint::new(x, y));

let normalized = stroke.normalize(width, height);

                  for point in normalized {
                      prop_assert!(point.x >= 0.0 && point.x <= 1.0);
                      prop_assert!(point.y >= 0.0 && point.y <= 1.0);
                  }
              }
          }

          #[cfg(feature = "proptest")]
          proptest! {
              #[test]
              fn prop_distance_threshold_filters(
                  x1 in 0.0f32..200.0,
                  y1 in 0.0f32..200.0,
                  dx in -1.0f32..1.0,
                  dy in -1.0f32..1.0
              ) {
                  // Property: Points within threshold distance should be filtered
                  let p1 = IcedPoint::new(x1, y1);
                  let p2 = IcedPoint::new(x1 + dx, y1 + dy);
                  let threshold = 2.0;

                  let distance = calculate_distance(p1, p2);
                  let should_add = distance >= threshold;

                  prop_assert_eq!(should_add, should_add_point(p1, p2, threshold));
              }
          }

          #[cfg(feature = "proptest")]
          proptest! {
              #[test]
              fn prop_timestamp_count_matches_points(
                  points in prop::collection::vec((0.0f32..200.0, 0.0f32..200.0), 1..50)
              ) {
                  // Property: Every point must have exactly one timestamp
                  let mut stroke = Stroke::new();

                  for (x, y) in points {
                      stroke.add_point(IcedPoint::new(x, y));
                  }

                  prop_assert_eq!(stroke.points.len(), stroke.timestamps.len());
              }
          }

          #[cfg(feature = "proptest")]
          proptest! {
              #[test]
              fn prop_undo_reduces_count(
                  stroke_count in 1usize..10
              ) {
                  // Property: Undo always reduces stroke count by exactly one
                  let mut state = DrawingState::new();

                  for i in 0..stroke_count {
                      state.start_stroke(IcedPoint::new(i as f32 * 10.0, i as f32 * 10.0));
                      state.complete_stroke();
                  }

                  let before = state.completed_strokes.len();
                  state.undo();
                  let after = state.completed_strokes.len();

                  prop_assert_eq!(after, before - 1);
              }
          }

          // Helper functions for property tests
          fn calculate_distance(p1: IcedPoint, p2: IcedPoint) -> f32 {
              let dx = p2.x - p1.x;
              let dy = p2.y - p1.y;
              (dx * dx + dy * dy).sqrt()
          }

          fn should_add_point(last_point: IcedPoint, new_point: IcedPoint, threshold: f32) -> bool {
              calculate_distance(last_point, new_point) >= threshold
          }
      }

  coverage:
    - "Stroke creation and initialization with empty vectors"
    - "Adding single and multiple points to strokes"
    - "Stroke coordinate normalization to 0-1 range"
    - "Normalization preserves aspect ratio correctly"
    - "Canvas boundary normalization for corner points"
    - "Euclidean distance calculation between points"
    - "Distance threshold filtering for noise reduction"
    - "Distance threshold maintains consistent point spacing"
    - "DrawingState initialization to idle state"
    - "Starting a new stroke with initial point"
    - "Adding points to current active stroke"
    - "Completing a stroke moves it to history"
    - "Clear operation removes all strokes and resets state"
    - "Undo operation removes most recent stroke"
    - "Undo on empty state does nothing gracefully"
    - "Export stroke data from empty canvas"
    - "Export multiple strokes with normalization"
    - "Multiple strokes maintain chronological sequence"
    - "Stroke continuity without gaps between points"
    - "Timestamp recording with monotonic increase"
    - "Complete drawing flow from start to export"
    - "Multiple sequential stroke handling"
    - "Clear operation during active drawing"
    - "Sequential undo operations with edge cases"
    - "Property: Normalization always produces [0,1] coordinate bounds"
    - "Property: Distance threshold correctly filters close points"
    - "Property: Timestamp count always matches point count"
    - "Property: Undo reduces stroke count by exactly one"

dependencies:
  depends_on:
    - task_id: 1
      reason: "Requires Canvas widget support from iced foundation for implementing canvas::Program trait and rendering functionality"

  depended_upon_by:
    - task_id: 8
      reason: "Practice mode requires drawing canvas for handwriting exercises and user interaction"
    - task_id: 9
      reason: "Recognition system needs stroke data format (normalized Vec<Vec<Point>>) for character validation"

  external:
    - name: "iced::widget::canvas"
      type: "module"
      status: "to be imported"
    - name: "iced::widget::canvas::Program"
      type: "trait"
      status: "to be imported"
    - name: "iced::Point"
      type: "struct"
      status: "to be imported"
    - name: "iced::mouse"
      type: "module"
      status: "to be imported"
    - name: "iced::Color"
      type: "struct"
      status: "to be imported"
    - name: "lyon::path::Path"
      type: "struct"
      status: "to be imported"
    - name: "lyon::path::Builder"
      type: "struct"
      status: "to be imported"
    - name: "std::time::SystemTime"
      type: "struct"
      status: "already exists"

old workflow:

task:
  id: 5
  name: "Handwriting Canvas and Input System"

context:
  description: |
    This task implements an interactive drawing canvas that captures user handwriting
    input through mouse or touch interactions. The system records stroke sequences with
    precise timestamps, provides real-time visual feedback during drawing, and outputs
    normalized stroke data suitable for character recognition systems.
    
    The canvas is a core component of the handwriting practice system, enabling users
    to draw Chinese characters for recognition and feedback. It must feel responsive
    and natural, rendering smooth anti-aliased strokes that provide immediate visual
    confirmation of input. The implementation uses iced's canvas::Program trait to
    handle interactive drawing with proper state management.
    
    Stroke data is recorded as sequences of points with timestamps, enabling future
    enhancements like velocity analysis and stroke order validation. The system applies
    intelligent filtering (2px distance threshold) to reduce noise while maintaining
    stroke fidelity. Coordinate normalization to 0-1 range ensures recognition algorithms
    can process input independent of canvas size.
    
    The architecture separates concerns cleanly: HandwritingCanvas handles widget-level
    integration with iced, DrawingState manages stroke history and drawing state machine,
    and Stroke encapsulates individual stroke data with normalization capabilities.

  key_points:
    - "Implements iced canvas::Program trait for interactive drawing with mouse/touch events"
    - "State machine tracks drawing lifecycle: idle → drawing → stroke complete"
    - "Distance threshold (2px) prevents excessive point density from noisy input"
    - "Stroke rendering uses lyon Path with round caps/joins for natural appearance"
    - "Coordinate normalization to 0-1 range enables size-independent recognition"
    - "Timestamp recording per point enables future velocity-based analysis"
    - "Clear separation between drawing state management and widget presentation"
    - "Visual differentiation between active drawing stroke and completed strokes"

files:
  - path: "src/ui/handwriting_canvas.rs"
    description: "Implements HandwritingCanvas widget with canvas::Program trait for interactive drawing input"
  
  - path: "src/ui/drawing_state.rs"
    description: "Maintains stroke history, current drawing state, and stroke data structures"
  
  - path: "src/ui/mod.rs"
    description: "Module declaration file to expose handwriting_canvas and drawing_state modules"
  
  - path: "src/types/stroke.rs"
    description: "Defines Stroke and Point data structures for stroke recording and normalization"
  
  - path: "tests/handwriting_canvas_tests.rs"
    description: "Integration tests for canvas input handling, stroke recording, and rendering behavior"

functions:
  - file: "src/ui/canvas.rs"
    items:
      - type: "module_declaration"
        name: "canvas"
        description: "Module containing handwriting canvas implementation for capturing and rendering stroke input"
      
      - type: "struct"
        name: "HandwritingCanvas"
        description: "Canvas widget that captures mouse/touch input and renders handwriting strokes in real-time"
        invariants: "State is never null; strokes list is ordered chronologically"
      
      - type: "struct"
        name: "DrawingState"
        description: "Maintains the complete drawing state including stroke history and current active stroke"
        invariants: "current_stroke is Some only when is_drawing is true"
      
      - type: "struct"
        name: "Stroke"
        description: "Represents a single continuous stroke as a sequence of points with timestamp"
        invariants: "Points vector is never empty for a completed stroke; timestamps are monotonically increasing"
      
      - type: "struct"
        name: "Point"
        description: "Represents a single point in a stroke with x, y coordinates and timestamp"
        invariants: "Coordinates are within canvas bounds when recorded"
      
      - type: "enum"
        name: "Message"
        description: "Messages for canvas interaction events"
      
      - type: "enum_variant"
        name: "Message::StartStroke"
        description: "Initiates a new stroke at the given position"
        preconditions: "No stroke is currently active"
        postconditions: "A new stroke is started in DrawingState"
      
      - type: "enum_variant"
        name: "Message::AddPoint"
        description: "Adds a point to the current active stroke"
        preconditions: "A stroke is currently active (is_drawing is true)"
        postconditions: "Point is added to current stroke if distance threshold is met"
      
      - type: "enum_variant"
        name: "Message::EndStroke"
        description: "Completes the current stroke and adds it to stroke history"
        preconditions: "A stroke is currently active"
        postconditions: "Current stroke is moved to completed strokes list"
      
      - type: "enum_variant"
        name: "Message::Clear"
        description: "Removes all strokes from the canvas"
        postconditions: "All strokes are removed; canvas is blank"
      
      - type: "enum_variant"
        name: "Message::Undo"
        description: "Removes the most recently completed stroke"
        postconditions: "Last stroke is removed from strokes list if any exist"
      
      - type: "function"
        name: "HandwritingCanvas::new"
        description: "Creates a new HandwritingCanvas instance with empty state"
        postconditions: "Returns initialized canvas with no strokes"
      
      - type: "function"
        name: "HandwritingCanvas::clear"
        description: "Removes all strokes from the canvas"
        postconditions: "All stroke data is cleared"
      
      - type: "function"
        name: "HandwritingCanvas::undo"
        description: "Removes the last completed stroke"
        postconditions: "Most recent stroke is removed if strokes exist"
      
      - type: "function"
        name: "HandwritingCanvas::get_strokes"
        description: "Returns the current stroke data as a reference"
        postconditions: "Returns immutable reference to stroke vector"
      
      - type: "function"
        name: "HandwritingCanvas::export_normalized_strokes"
        description: "Exports stroke data normalized to 0-1 coordinate range"
        postconditions: "Returns Vec<Vec<(f32, f32)>> with normalized coordinates"
      
      - type: "trait_impl"
        name: "canvas::Program for HandwritingCanvas"
        description: "Implements iced canvas::Program trait for interactive drawing functionality"
      
      - type: "method"
        name: "canvas::Program::update"
        description: "Handles canvas messages and updates drawing state"
        preconditions: "Valid Message is provided"
        postconditions: "State is updated according to message type"
      
      - type: "method"
        name: "canvas::Program::draw"
        description: "Renders all strokes and current drawing stroke to the canvas"
        postconditions: "All strokes are rendered with anti-aliasing and round caps"
      
      - type: "method"
        name: "canvas::Program::mouse_interaction"
        description: "Provides cursor feedback based on canvas state"
        postconditions: "Returns appropriate cursor style"
      
      - type: "function"
        name: "DrawingState::new"
        description: "Creates a new empty drawing state"
        postconditions: "Returns initialized state with empty stroke list"
      
      - type: "function"
        name: "DrawingState::start_stroke"
        description: "Begins a new stroke at the given position with timestamp"
        preconditions: "No stroke is currently active"
        postconditions: "current_stroke is Some with initial point; is_drawing is true"
      
      - type: "function"
        name: "DrawingState::add_point"
        description: "Adds a point to the current stroke if distance threshold is met"
        preconditions: "A stroke is currently active"
        postconditions: "Point is added if distance from last point exceeds threshold (2px)"
      
      - type: "function"
        name: "DrawingState::end_stroke"
        description: "Completes the current stroke and adds it to the stroke history"
        preconditions: "A stroke is currently active"
        postconditions: "current_stroke is None; completed stroke is added to strokes list"
      
      - type: "function"
        name: "DrawingState::clear_all"
        description: "Removes all strokes and resets state"
        postconditions: "strokes is empty; current_stroke is None; is_drawing is false"
      
      - type: "function"
        name: "DrawingState::undo_last"
        description: "Removes the most recent completed stroke"
        postconditions: "Last element removed from strokes if list is non-empty"
      
      - type: "function"
        name: "Stroke::new"
        description: "Creates a new stroke starting at the given point"
        postconditions: "Returns stroke with single initial point"
      
      - type: "function"
        name: "Stroke::add_point"
        description: "Appends a point to the stroke"
        postconditions: "Point is added to points vector"
      
      - type: "function"
        name: "Stroke::to_path"
        description: "Converts stroke points into a lyon Path for rendering"
        preconditions: "Stroke has at least one point"
        postconditions: "Returns Path with line_to segments connecting all points"
      
      - type: "function"
        name: "Stroke::normalize"
        description: "Normalizes stroke coordinates to 0-1 range based on canvas bounds"
        preconditions: "canvas_width and canvas_height are positive"
        postconditions: "Returns vector of normalized (x, y) tuples"
      
      - type: "function"
        name: "Point::new"
        description: "Creates a new point with coordinates and timestamp"
        postconditions: "Returns initialized Point"
      
      - type: "function"
        name: "Point::distance_to"
        description: "Calculates Euclidean distance to another point"
        postconditions: "Returns non-negative distance value"
      
      - type: "function"
        name: "render_stroke"
        description: "Helper function to render a stroke with specified style to the frame"
        preconditions: "Stroke has at least 2 points for visible rendering"
        postconditions: "Stroke is drawn to frame with anti-aliasing, round caps and joins"
      
      - type: "constant"
        name: "DISTANCE_THRESHOLD"
        description: "Minimum distance (2.0 pixels) between consecutive points to reduce noise"
      
      - type: "constant"
        name: "STROKE_WIDTH"
        description: "Default width (3.0 pixels) for rendered strokes"
      
      - type: "constant"
        name: "DRAWING_COLOR"
        description: "Color used for the current active stroke being drawn"
      
      - type: "constant"
        name: "COMPLETED_COLOR"
        description: "Color used for completed strokes"

  - file: "tests/canvas_tests.rs"
    items:
      - type: "module_declaration"
        name: "canvas_tests"
        description: "Integration tests for handwriting canvas functionality"
      
      - type: "function"
        name: "test_stroke_creation"
        description: "Tests that strokes are created correctly with initial points"
        postconditions: "Stroke contains expected initial point"
      
      - type: "function"
        name: "test_distance_threshold"
        description: "Verifies that points closer than threshold are not added"
        postconditions: "Only points exceeding distance threshold are added to stroke"
      
      - type: "function"
        name: "test_stroke_completion"
        description: "Tests that ending a stroke moves it to completed list"
        postconditions: "Current stroke becomes None; strokes list grows by one"
      
      - type: "function"
        name: "test_clear_functionality"
        description: "Verifies clear removes all strokes"
        postconditions: "After clear, strokes list is empty"
      
      - type: "function"
        name: "test_undo_functionality"
        description: "Tests that undo removes most recent stroke"
        postconditions: "Last stroke is removed; count decreases by one"
      
      - type: "function"
        name: "test_normalization"
        description: "Verifies stroke coordinate normalization to 0-1 range"
        postconditions: "All normalized coordinates are between 0.0 and 1.0"
      
      - type: "function"
        name: "test_stroke_continuity"
        description: "Tests that strokes remain continuous without gaps"
        postconditions: "Each point connects to previous point in sequence"
      
      - type: "function"
        name: "test_multiple_strokes"
        description: "Verifies handling of multiple sequential strokes"
        postconditions: "All strokes are maintained in chronological order"

formal_verification:
  needed: false
  level: "None"
  explanation: |
    Formal verification is not required for the handwriting canvas system because:
    
    1. The system is primarily an interactive UI component with non-critical behavior.
       Errors in stroke rendering or input handling would be immediately visible to users
       and do not pose safety, security, or data integrity risks.
    
    2. The correctness properties (stroke continuity, point sampling, coordinate 
       normalization) can be effectively validated through integration testing with
       simulated input events and visual regression testing.
    
    3. The state machine (idle → drawing → stroke complete) is simple with only three
       states and deterministic transitions triggered by mouse/touch events. The logic
       is straightforward enough that standard unit and integration tests provide
       adequate confidence.
    
    4. The canvas operates on local user input data that can be easily regenerated.
       Unlike systems handling persistent data, financial transactions, or safety-critical
       operations, incorrect behavior here only affects the current drawing session.
    
    5. The distance threshold filtering and coordinate normalization are simple arithmetic
       operations that can be thoroughly tested with property-based testing if needed,
       but don't require formal proof of correctness.
    
    Standard testing approaches (unit tests for stroke data structures, integration tests
    for event handling, property tests for normalization invariants) are sufficient and
    more cost-effective than formal verification for this UI component.

tests:
  strategy:
    approach: "mixed (unit + integration + property-based)"
    rationale:
      - "Unit tests verify stroke data structures, point normalization, and distance thresholding logic independently"
      - "Integration tests validate the complete input-to-render pipeline with simulated mouse events"
      - "Canvas widget requires integration testing since it implements canvas::Program trait and depends on event handling state machine"
      - "Stroke rendering and visual feedback require integration with iced's canvas system"
      - "Property-based testing ensures distance threshold and normalization work correctly across all input ranges"

  implementation:
    file: "src/ui/handwriting_canvas.rs"
    location: "in existing test module"
    code: |
      #[cfg(test)]
      mod tests {
          use super::*;
          use iced::Point as IcedPoint;
          use std::time::{Duration, SystemTime};

          // Unit tests for Stroke data structure
          #[test]
          fn test_stroke_creation_empty() {
              let stroke = Stroke::new();
              assert!(stroke.points.is_empty());
              assert!(stroke.timestamps.is_empty());
          }

          #[test]
          fn test_stroke_add_point() {
              let mut stroke = Stroke::new();
              let point = IcedPoint::new(10.0, 20.0);
              stroke.add_point(point);
              
              assert_eq!(stroke.points.len(), 1);
              assert_eq!(stroke.timestamps.len(), 1);
              assert_eq!(stroke.points[0], point);
          }

          #[test]
          fn test_stroke_normalization() {
              let mut stroke = Stroke::new();
              stroke.add_point(IcedPoint::new(0.0, 0.0));
              stroke.add_point(IcedPoint::new(100.0, 50.0));
              stroke.add_point(IcedPoint::new(200.0, 200.0));
              
              let normalized = stroke.normalize(200.0, 200.0);
              
              assert_eq!(normalized.len(), 3);
              assert_eq!(normalized[0].x, 0.0);
              assert_eq!(normalized[0].y, 0.0);
              assert_eq!(normalized[1].x, 0.5);
              assert_eq!(normalized[1].y, 0.25);
              assert_eq!(normalized[2].x, 1.0);
              assert_eq!(normalized[2].y, 1.0);
          }

          #[test]
          fn test_distance_threshold_filtering() {
              let point1 = IcedPoint::new(10.0, 10.0);
              let point2 = IcedPoint::new(10.5, 10.5);
              let point3 = IcedPoint::new(15.0, 15.0);
              
              assert!(!should_add_point(point1, point2, 2.0));
              assert!(should_add_point(point1, point3, 2.0));
          }

          #[test]
          fn test_distance_calculation() {
              let p1 = IcedPoint::new(0.0, 0.0);
              let p2 = IcedPoint::new(3.0, 4.0);
              let distance = calculate_distance(p1, p2);
              
              assert!((distance - 5.0).abs() < 0.001);
          }

          #[test]
          fn test_drawing_state_initial() {
              let state = DrawingState::new();
              
              assert!(!state.is_drawing);
              assert!(state.current_stroke.is_none());
              assert!(state.completed_strokes.is_empty());
          }

          #[test]
          fn test_drawing_state_start_stroke() {
              let mut state = DrawingState::new();
              state.start_stroke(IcedPoint::new(10.0, 10.0));
              
              assert!(state.is_drawing);
              assert!(state.current_stroke.is_some());
              if let Some(ref stroke) = state.current_stroke {
                  assert_eq!(stroke.points.len(), 1);
              }
          }

          #[test]
          fn test_drawing_state_add_point_to_current_stroke() {
              let mut state = DrawingState::new();
              state.start_stroke(IcedPoint::new(10.0, 10.0));
              state.add_point(IcedPoint::new(15.0, 15.0));
              
              if let Some(ref stroke) = state.current_stroke {
                  assert_eq!(stroke.points.len(), 2);
              } else {
                  panic!("Current stroke should exist");
              }
          }

          #[test]
          fn test_drawing_state_complete_stroke() {
              let mut state = DrawingState::new();
              state.start_stroke(IcedPoint::new(10.0, 10.0));
              state.add_point(IcedPoint::new(15.0, 15.0));
              state.complete_stroke();
              
              assert!(!state.is_drawing);
              assert!(state.current_stroke.is_none());
              assert_eq!(state.completed_strokes.len(), 1);
              assert_eq!(state.completed_strokes[0].points.len(), 2);
          }

          #[test]
          fn test_drawing_state_clear() {
              let mut state = DrawingState::new();
              state.start_stroke(IcedPoint::new(10.0, 10.0));
              state.complete_stroke();
              state.start_stroke(IcedPoint::new(20.0, 20.0));
              state.complete_stroke();
              
              assert_eq!(state.completed_strokes.len(), 2);
              
              state.clear();
              
              assert!(state.completed_strokes.is_empty());
              assert!(!state.is_drawing);
              assert!(state.current_stroke.is_none());
          }

          #[test]
          fn test_drawing_state_undo() {
              let mut state = DrawingState::new();
              state.start_stroke(IcedPoint::new(10.0, 10.0));
              state.complete_stroke();
              state.start_stroke(IcedPoint::new(20.0, 20.0));
              state.complete_stroke();
              
              assert_eq!(state.completed_strokes.len(), 2);
              
              state.undo();
              
              assert_eq!(state.completed_strokes.len(), 1);
          }

          #[test]
          fn test_undo_empty_does_nothing() {
              let mut state = DrawingState::new();
              state.undo();
              
              assert!(state.completed_strokes.is_empty());
          }

          #[test]
          fn test_export_stroke_data_empty() {
              let state = DrawingState::new();
              let exported = state.export_stroke_data(200.0, 200.0);
              
              assert!(exported.is_empty());
          }

          #[test]
          fn test_export_stroke_data_multiple_strokes() {
              let mut state = DrawingState::new();
              
              state.start_stroke(IcedPoint::new(0.0, 0.0));
              state.add_point(IcedPoint::new(100.0, 100.0));
              state.complete_stroke();
              
              state.start_stroke(IcedPoint::new(50.0, 50.0));
              state.add_point(IcedPoint::new(150.0, 150.0));
              state.complete_stroke();
              
              let exported = state.export_stroke_data(200.0, 200.0);
              
              assert_eq!(exported.len(), 2);
              assert_eq!(exported[0].len(), 2);
              assert_eq!(exported[1].len(), 2);
              
              assert!(exported[0][0].x >= 0.0 && exported[0][0].x <= 1.0);
              assert!(exported[0][0].y >= 0.0 && exported[0][0].y <= 1.0);
          }

          #[test]
          fn test_multiple_strokes_maintain_sequence() {
              let mut state = DrawingState::new();
              
              state.start_stroke(IcedPoint::new(10.0, 10.0));
              state.complete_stroke();
              
              state.start_stroke(IcedPoint::new(20.0, 20.0));
              state.complete_stroke();
              
              state.start_stroke(IcedPoint::new(30.0, 30.0));
              state.complete_stroke();
              
              assert_eq!(state.completed_strokes.len(), 3);
              assert_eq!(state.completed_strokes[0].points[0].x, 10.0);
              assert_eq!(state.completed_strokes[1].points[0].x, 20.0);
              assert_eq!(state.completed_strokes[2].points[0].x, 30.0);
          }

          #[test]
          fn test_stroke_continuity_no_gaps() {
              let mut stroke = Stroke::new();
              let points = vec![
                  IcedPoint::new(10.0, 10.0),
                  IcedPoint::new(15.0, 15.0),
                  IcedPoint::new(20.0, 20.0),
                  IcedPoint::new(25.0, 25.0),
              ];
              
              for point in points.iter() {
                  stroke.add_point(*point);
              }
              
              assert_eq!(stroke.points.len(), 4);
              
              for (i, point) in points.iter().enumerate() {
                  assert_eq!(stroke.points[i], *point);
              }
          }

          #[test]
          fn test_timestamp_recording() {
              let mut stroke = Stroke::new();
              
              stroke.add_point(IcedPoint::new(10.0, 10.0));
              std::thread::sleep(Duration::from_millis(10));
              stroke.add_point(IcedPoint::new(20.0, 20.0));
              
              assert_eq!(stroke.timestamps.len(), 2);
              
              let t1 = stroke.timestamps[0];
              let t2 = stroke.timestamps[1];
              assert!(t2 > t1);
              
              let duration = t2.duration_since(t1).unwrap();
              assert!(duration.as_millis() >= 10);
          }

          #[test]
          fn test_distance_threshold_consistency() {
              let mut state = DrawingState::new();
              state.start_stroke(IcedPoint::new(10.0, 10.0));
              
              for i in 1..=20 {
                  let point = IcedPoint::new(10.0 + (i as f32) * 0.1, 10.0);
                  state.add_point_with_threshold(point, 2.0);
              }
              
              if let Some(ref stroke) = state.current_stroke {
                  assert!(stroke.points.len() < 20);
                  
                  for i in 1..stroke.points.len() {
                      let dist = calculate_distance(stroke.points[i-1], stroke.points[i]);
                      assert!(dist >= 1.9);
                  }
              }
          }

          #[test]
          fn test_normalization_preserves_aspect_ratio() {
              let mut stroke = Stroke::new();
              stroke.add_point(IcedPoint::new(0.0, 0.0));
              stroke.add_point(IcedPoint::new(100.0, 200.0));
              
              let normalized = stroke.normalize(200.0, 200.0);
              
              assert!((normalized[1].x - 0.5).abs() < 0.001);
              assert!((normalized[1].y - 1.0).abs() < 0.001);
          }

          #[test]
          fn test_canvas_bounds_normalization() {
              let mut stroke = Stroke::new();
              
              stroke.add_point(IcedPoint::new(0.0, 0.0));
              stroke.add_point(IcedPoint::new(200.0, 0.0));
              stroke.add_point(IcedPoint::new(200.0, 200.0));
              stroke.add_point(IcedPoint::new(0.0, 200.0));
              
              let normalized = stroke.normalize(200.0, 200.0);
              
              assert_eq!(normalized[0], IcedPoint::new(0.0, 0.0));
              assert_eq!(normalized[1], IcedPoint::new(1.0, 0.0));
              assert_eq!(normalized[2], IcedPoint::new(1.0, 1.0));
              assert_eq!(normalized[3], IcedPoint::new(0.0, 1.0));
          }

          fn simulate_mouse_drag(state: &mut DrawingState, start: IcedPoint, end: IcedPoint, steps: usize) {
              state.start_stroke(start);
              
              for i in 1..=steps {
                  let t = i as f32 / steps as f32;
                  let x = start.x + (end.x - start.x) * t;
                  let y = start.y + (end.y - start.y) * t;
                  state.add_point(IcedPoint::new(x, y));
              }
              
              state.complete_stroke();
          }

          #[test]
          fn test_integration_complete_drawing_flow() {
              let mut state = DrawingState::new();
              
              simulate_mouse_drag(
                  &mut state,
                  IcedPoint::new(10.0, 10.0),
                  IcedPoint::new(100.0, 100.0),
                  10
              );
              
              assert_eq!(state.completed_strokes.len(), 1);
              assert!(state.completed_strokes[0].points.len() > 0);
              
              let exported = state.export_stroke_data(200.0, 200.0);
              assert_eq!(exported.len(), 1);
              assert!(exported[0].len() > 0);
          }

          #[test]
          fn test_integration_multiple_stroke_sequence() {
              let mut state = DrawingState::new();
              
              simulate_mouse_drag(&mut state, IcedPoint::new(10.0, 10.0), IcedPoint::new(50.0, 50.0), 5);
              simulate_mouse_drag(&mut state, IcedPoint::new(60.0, 10.0), IcedPoint::new(100.0, 50.0), 5);
              simulate_mouse_drag(&mut state, IcedPoint::new(10.0, 60.0), IcedPoint::new(50.0, 100.0), 5);
              
              assert_eq!(state.completed_strokes.len(), 3);
              
              let exported = state.export_stroke_data(200.0, 200.0);
              assert_eq!(exported.len(), 3);
          }

          #[test]
          fn test_integration_clear_during_drawing() {
              let mut state = DrawingState::new();
              
              simulate_mouse_drag(&mut state, IcedPoint::new(10.0, 10.0), IcedPoint::new(50.0, 50.0), 5);
              state.start_stroke(IcedPoint::new(60.0, 60.0));
              state.add_point(IcedPoint::new(70.0, 70.0));
              
              state.clear();
              
              assert!(state.completed_strokes.is_empty());
              assert!(!state.is_drawing);
              assert!(state.current_stroke.is_none());
          }

          #[test]
          fn test_integration_undo_sequence() {
              let mut state = DrawingState::new();
              
              simulate_mouse_drag(&mut state, IcedPoint::new(10.0, 10.0), IcedPoint::new(50.0, 50.0), 5);
              simulate_mouse_drag(&mut state, IcedPoint::new(60.0, 10.0), IcedPoint::new(100.0, 50.0), 5);
              simulate_mouse_drag(&mut state, IcedPoint::new(10.0, 60.0), IcedPoint::new(50.0, 100.0), 5);
              
              assert_eq!(state.completed_strokes.len(), 3);
              
              state.undo();
              assert_eq!(state.completed_strokes.len(), 2);
              
              state.undo();
              assert_eq!(state.completed_strokes.len(), 1);
              
              state.undo();
              assert_eq!(state.completed_strokes.len(), 0);
              
              state.undo();
              assert_eq!(state.completed_strokes.len(), 0);
          }

          fn should_add_point(last_point: IcedPoint, new_point: IcedPoint, threshold: f32) -> bool {
              calculate_distance(last_point, new_point) >= threshold
          }

          fn calculate_distance(p1: IcedPoint, p2: IcedPoint) -> f32 {
              let dx = p2.x - p1.x;
              let dy = p2.y - p1.y;
              (dx * dx + dy * dy).sqrt()
          }
      }

      #[cfg(test)]
      mod property_tests {
          use super::*;
          use proptest::prelude::*;
          use iced::Point as IcedPoint;

          proptest! {
              #[test]
              fn prop_normalization_bounds(
                  x in 0.0f32..1000.0,
                  y in 0.0f32..1000.0,
                  width in 100.0f32..1000.0,
                  height in 100.0f32..1000.0
              ) {
                  let mut stroke = Stroke::new();
                  stroke.add_point(IcedPoint::new(x, y));
                  
                  let normalized = stroke.normalize(width, height);
                  
                  for point in normalized {
                      prop_assert!(point.x >= 0.0 && point.x <= 1.0);
                      prop_assert!(point.y >= 0.0 && point.y <= 1.0);
                  }
              }
          }

          proptest! {
              #[test]
              fn prop_distance_threshold_filters(
                  x1 in 0.0f32..200.0,
                  y1 in 0.0f32..200.0,
                  dx in -1.0f32..1.0,
                  dy in -1.0f32..1.0
              ) {
                  let p1 = IcedPoint::new(x1, y1);
                  let p2 = IcedPoint::new(x1 + dx, y1 + dy);
                  let threshold = 2.0;
                  
                  let distance = calculate_distance(p1, p2);
                  let should_add = distance >= threshold;
                  
                  prop_assert_eq!(should_add, should_add_point(p1, p2, threshold));
              }
          }

          proptest! {
              #[test]
              fn prop_timestamp_count_matches_points(
                  points in prop::collection::vec((0.0f32..200.0, 0.0f32..200.0), 1..50)
              ) {
                  let mut stroke = Stroke::new();
                  
                  for (x, y) in points {
                      stroke.add_point(IcedPoint::new(x, y));
                  }
                  
                  prop_assert_eq!(stroke.points.len(), stroke.timestamps.len());
              }
          }

          proptest! {
              #[test]
              fn prop_undo_reduces_count(
                  stroke_count in 1usize..10
              ) {
                  let mut state = DrawingState::new();
                  
                  for i in 0..stroke_count {
                      state.start_stroke(IcedPoint::new(i as f32 * 10.0, i as f32 * 10.0));
                      state.complete_stroke();
                  }
                  
                  let before = state.completed_strokes.len();
                  state.undo();
                  let after = state.completed_strokes.len();
                  
                  prop_assert_eq!(after, before - 1);
              }
          }

          fn calculate_distance(p1: IcedPoint, p2: IcedPoint) -> f32 {
              let dx = p2.x - p1.x;
              let dy = p2.y - p1.y;
              (dx * dx + dy * dy).sqrt()
          }

          fn should_add_point(last_point: IcedPoint, new_point: IcedPoint, threshold: f32) -> bool {
              calculate_distance(last_point, new_point) >= threshold
          }
      }

  coverage:
    - "Stroke creation and initialization"
    - "Adding points to strokes"
    - "Stroke normalization to 0-1 coordinate range"
    - "Distance threshold filtering for point sampling"
    - "Distance calculation between points"
    - "DrawingState initialization"
    - "Starting a new stroke"
    - "Adding points to current stroke"
    - "Completing a stroke"
    - "Clearing all strokes"
    - "Undo operation for last stroke"
    - "Undo on empty state does nothing"
    - "Exporting stroke data for empty canvas"
    - "Exporting multiple strokes with normalization"
    - "Multiple strokes maintain correct sequence"
    - "Stroke continuity without gaps"
    - "Timestamp recording for each point"
    - "Distance threshold maintains consistent spacing"
    - "Normalization preserves aspect ratio"
    - "Canvas boundary normalization"
    - "Complete drawing flow from start to export"
    - "Multiple stroke sequence handling"
    - "Clear operation during active drawing"
    - "Sequential undo operations"
    - "Property: Normalization always produces [0,1] bounds"
    - "Property: Distance threshold filters close points"
    - "Property: Timestamp count matches point count"
    - "Property: Undo reduces stroke count correctly"

dependencies:
  depends_on:
    - task_id: 1
      reason: "Requires iced framework foundation including Canvas widget support and canvas::Program trait"

  depended_upon_by:
    - task_id: 8
      reason: "Practice mode integrates HandwritingCanvas for user input during character practice exercises"
    - task_id: 9
      reason: "Recognition system consumes normalized stroke data exported from canvas for character matching"

  external:
    - name: "iced::widget::canvas"
      type: "module"
      status: "already exists"
    - name: "iced::widget::canvas::Program"
      type: "trait"
      status: "already exists"
    - name: "iced::Point"
      type: "struct"
      status: "already exists"
    - name: "lyon::path::Path"
      type: "struct"
      status: "already exists"
    - name: "lyon::path::Builder"
      type: "struct"
      status: "already exists"
    - name: "std::time::SystemTime"
      type: "struct"
      status: "already exists"

