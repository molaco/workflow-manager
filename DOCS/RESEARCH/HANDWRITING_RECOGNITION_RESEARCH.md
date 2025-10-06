# Handwriting Recognition Research for Hiragana/Katakana in Rust

## Executive Summary

This document outlines methods for implementing handwriting/visual recognition of hiragana and katakana characters in Rust, covering canvas drawing, stroke matching algorithms, and character comparison techniques.

---

## 1. Canvas Drawing for User Input Capture

### Recommended Rust GUI/Canvas Libraries

#### **egui** (Recommended)
- Cross-platform immediate-mode GUI library
- Built-in `Painter` struct for drawing operations
- Supports shapes: `LineSegment`, `Rect`, `Circle`
- Ideal for drawing canvas with stroke capture
- Mouse/touch event handling built-in

#### **iced**
- Cross-platform, Elm-inspired GUI library
- Modular architecture with canvas widget
- Good for production applications
- Native rendering with custom drawing

#### **rust_canvas** (WebAssembly)
- Browser-based 2D canvas graphics
- Full 2D canvas API support
- Mouse/keyboard event handling
- Trait-based event system
- Best for web-based applications

#### **Slint**
- Polished traditional GUI framework
- Uses DSL for UI definitions
- Production-ready
- Good documentation

### Input Capture Strategy
- Capture mouse/touch coordinates as stroke points
- Record timestamps for stroke velocity analysis
- Store strokes as sequences of (x, y, timestamp) tuples
- Normalize stroke data for scale/position invariance

---

## 2. Stroke Matching Algorithms

### Dynamic Time Warping (DTW)
**Purpose:** Measure similarity between temporal sequences

**Key Features:**
- Handles speed variations in writing
- O(N²) time complexity
- Matches distinctive patterns regardless of timing
- Widely used in handwriting/signature recognition

**Application:**
- Compare user stroke sequence with reference patterns
- Allow for speed/timing variations
- Match strokes that may have slight distortions

### Dynamic Positional Warping (DPW)
**Purpose:** Specialized DTW variant for 2D handwriting

**Advantages:**
- Addresses unintended DTW correspondences in 2D signals
- Allows subsignal translations without additional cost
- Finds and matches similar subsignals effectively
- Better suited for handwriting than standard DTW

**Implementation Strategy:**
- Use for comparing full character strokes
- Handle multi-stroke characters (e.g., さ, き, む)
- Account for stroke order variations

### Hausdorff Distance
**Purpose:** Shape matching between point/curve sets

**Key Features:**
- Efficient quadratic-time computation
- Handles partial matches well
- Good for graph-based representations
- 12.9x speedup vs full graph edit distance

**Application:**
- Compare stroke shapes as polylines
- Graph-based character representation
- Combine with Hausdorff Edit Distance (HED)
- Keyword spotting in handwriting

### Stroke Correspondence Algorithms
**Purpose:** Match strokes between input and reference

**Approaches:**
- One-to-one stroke correspondence problem
- Handle stroke concatenation/splitting
- Three types of inter-stroke distances:
  1. Shape similarity
  2. Positional relationship
  3. Directional alignment
- Stroke-order independent matching

---

## 3. OCR Libraries and Pattern Matching

### Rust OCR Libraries

#### **ocrs** (Limited)
- Pure Rust OCR library
- **Limitation:** Latin alphabet only (no Japanese support)
- Not suitable for hiragana/katakana

#### **ort** (Recommended for ONNX)
- Fast ML inference for ONNX models
- Hardware-accelerated (CUDA, TensorRT, OpenVINO)
- Can run pre-trained Japanese OCR models
- Actively maintained, production-ready
- GitHub: pykeio/ort

#### **RTen** (Pure Rust ONNX)
- Pure Rust ONNX runtime
- Exports from PyTorch/other frameworks
- No C dependencies
- Good for cross-platform deployment

#### **wonnx**
- GPU-based ONNX runtime via wgpu
- Universal GPU acceleration
- Good for real-time inference

### Pattern Matching Approaches

#### Template Matching
**Traditional Approach:**
- Pre-defined templates for each character
- Distance minimization function
- 89.8% accuracy reported for hiragana/katakana

**Limitations:**
- Sensitive to stroke order variations
- Struggles with shape distortions
- Multi-modal character distributions (hentaigana)

**Improvements:**
- Stroke-order independent matching
- Multiple templates per character
- Adaptive normalization

#### Deep Learning (CNN-based)
**Modern Approach:**
- Convolutional Neural Networks
- Automatic feature extraction
- Handles stroke order/shape variations
- Ensemble methods for improved accuracy

**Datasets:**
- Kuzushiji-MNIST: 70k 28x28 images, 10 hiragana classes
- Kuzushiji-49: 49 hiragana characters
- Kuzushiji-Kanji: Kanji character dataset
- Perfectly balanced train/test splits

---

## 4. Character Comparison Techniques

### Feature Extraction Methods

#### Histogram of Oriented Gradients (HOG)
**Purpose:** Extract gradient-based features from images

**How it Works:**
1. Convert to grayscale
2. Compute gradients (magnitude + direction)
3. Create histograms in local cells
4. Block normalization for illumination invariance

**Performance:**
- 99.36% accuracy on MNIST (digit recognition)
- 98.8% on handwritten letters (with SVM)
- Invariant to geometric/photometric transforms
- Effective for handwritten character features

**Implementation:**
- Cell size: 14x14 for 28x28 images
- 9 bins for 180° gradient range (20° per bin)
- L2-norm block normalization
- Combine with SVM/k-NN classifiers

#### Directional Features
**Purpose:** Capture stroke direction information

**Application:**
- Critical for hiragana/katakana distinction
- Low-stroke kanji recognition
- Improves multi-character classification

#### Stroke-Based Features
**Extraction Process:**
1. Find Center of Gravity (CoG)
2. Divide into quadrants
3. Calculate CoG per quadrant
4. Locate conjunction points (stroke intersections)
5. Find endpoints
6. Measure Euclidean distances from CoG

**Feature Vector:**
- CoG coordinates
- Conjunction point distances
- Endpoint distances
- Quadrant-based features

### Similarity Metrics

#### Cosine Similarity
**Purpose:** Measure vector orientation similarity

**Formula:** cos(θ) = (A · B) / (||A|| × ||B||)

**Range:** -1 (opposite) to +1 (identical)

**Application:**
- Compare HOG feature vectors
- High-dimensional feature spaces
- Magnitude-independent comparison
- Good for normalized features

**Limitation:** Not a proper distance metric (violates triangle inequality)

#### Euclidean Distance
**Purpose:** Geometric distance in feature space

**Application:**
- Template matching
- Feature vector comparison
- CoG-based distance measurements

**Use When:** Magnitude matters (spatial measurements)

#### Hausdorff Distance (for shapes)
**Purpose:** Set-to-set distance for shapes

**Application:**
- Stroke shape comparison
- Polyline matching
- Partial stroke matching

---

## 5. Image Processing Libraries

### Core Rust Libraries

#### **imageproc**
- Built on `image` crate
- Performant, well-documented
- Computer vision foundation
- Edge detection, filtering, transforms

#### **image** crate
- Core image handling
- Multiple format support (PNG, JPEG, etc.)
- Clean API: `image::open()`, `image::io::Reader`

#### **kornia-rs** (Recommended for CV)
- Pure Rust 3D computer vision library
- Memory and thread-safe (Rust ownership model)
- Modular crate architecture:
  - `kornia-tensor`: Type-safe tensor operations
  - `kornia-imgproc`: Filtering, geometric transforms
- Real-time, safety-critical ready
- No C++ dependencies (unlike OpenCV)

#### **OpenCV bindings**
- Access to full OpenCV library
- Comprehensive CV algorithms
- Mature, battle-tested
- C++ dependency required

### Additional Tools
- **zune-image**: Decode, manipulate, encode images
- **ndarray-vision**: CV built on ndarray
- **tract**: Neural network inference (ONNX/TensorFlow)

---

## 6. Recommended Implementation Strategy

### Phase 1: Basic Stroke Capture
1. Use **egui** for canvas drawing UI
2. Capture strokes as point sequences
3. Implement stroke preprocessing:
   - Normalization (position, scale)
   - Smoothing (reduce noise)
   - Resampling (uniform point spacing)

### Phase 2: Feature Extraction
1. Use **imageproc** or **kornia-rs** for image processing
2. Implement HOG feature extraction:
   - Convert stroke to raster image (28x28)
   - Calculate gradients
   - Build histograms
   - Normalize blocks
3. Extract stroke-based features:
   - CoG calculations
   - Conjunction/endpoint detection

### Phase 3: Matching Engine
1. **Template Matching:**
   - Pre-compute features for all hiragana/katakana
   - Use Euclidean distance or cosine similarity
   - Top-k candidates retrieval

2. **DTW/DPW Matching:**
   - Implement Dynamic Positional Warping
   - Compare stroke sequences
   - Handle stroke order variations

3. **ML-based (Advanced):**
   - Use **ort** to load pre-trained ONNX model
   - Train CNN on Kuzushiji-MNIST dataset
   - Deploy for real-time inference

### Phase 4: Recognition Pipeline
```rust
// Pseudocode flow
user_stroke -> preprocess() -> extract_features()
  -> match_templates() -> rank_candidates() -> return_top_match
```

### Performance Optimization
- Pre-compute all template features (startup)
- Use parallel matching (rayon)
- Implement early rejection (quick filters)
- Cache recent results
- GPU acceleration via wgpu/kornia-rs

---

## 7. Key Challenges for Hiragana/Katakana

### Multi-modal Distributions
- Hentaigana: Multiple valid forms per character
- Handwriting variations (cursive, print)
- Personal style differences

### Stroke Characteristics
- Similar shapes (e.g., ソ vs ン, シ vs ツ)
- Stroke order variations
- Stroke count ambiguity (connections)
- Size/aspect ratio differences

### Solutions
1. Multiple templates per character
2. Stroke-order independent algorithms
3. Directional feature emphasis
4. Ensemble classifiers
5. Confusion matrix analysis for similar pairs

---

## 8. Datasets for Training/Testing

### Kuzushiji-MNIST
- 70,000 images (28x28 grayscale)
- 10 hiragana classes
- 6k train / 1k test per class
- Balanced dataset
- Drop-in MNIST replacement

### Kuzushiji-49
- 49 hiragana characters
- Larger character set
- Historical cursive styles

### Kuzushiji-Kanji
- Kanji character dataset
- Complex stroke patterns
- Transfer learning potential

### Access
- GitHub: rois-codh/kmnist
- Kaggle datasets
- MNIST/NumPy formats
- TensorFlow Datasets

---

## 9. Recommended Tech Stack

### Minimal Stack (Template Matching)
```toml
[dependencies]
egui = "0.29"           # Canvas UI
image = "0.25"          # Image handling
imageproc = "0.25"      # Feature extraction
nalgebra = "0.33"       # Linear algebra
```

### Advanced Stack (ML-based)
```toml
[dependencies]
egui = "0.29"           # Canvas UI
kornia-rs = "0.1"       # Computer vision
ort = "2.0"             # ONNX inference
ndarray = "0.16"        # N-dimensional arrays
image = "0.25"          # Image I/O
```

### Cross-platform Considerations
- egui: Native + WebAssembly support
- rust_canvas: WebAssembly only
- kornia-rs: Pure Rust, cross-platform
- ort: Multi-platform ML inference

---

## 10. Next Steps

1. **Prototype Canvas:**
   - Implement basic egui drawing canvas
   - Capture and visualize strokes
   - Export stroke data

2. **Feature Extraction:**
   - Implement HOG extraction
   - Test on sample characters
   - Validate feature discrimination

3. **Template Database:**
   - Create/download hiragana/katakana templates
   - Pre-compute features
   - Build similarity index

4. **Recognition Engine:**
   - Implement DTW/DPW
   - Compare with template matching
   - Evaluate accuracy on test set

5. **ML Integration (Optional):**
   - Train CNN on Kuzushiji-MNIST
   - Export to ONNX
   - Integrate with ort runtime
   - Benchmark performance

---

## References

- Kuzushiji-MNIST: https://github.com/rois-codh/kmnist
- egui: https://github.com/emilk/egui
- kornia-rs: https://github.com/kornia/kornia-rs
- ort: https://github.com/pykeio/ort
- imageproc: https://github.com/image-rs/imageproc

## Research Papers
- "Recognition of Handwritten Japanese Characters Using Ensemble of CNNs" (2023)
- "Handwritten recognition of Hiragana and Katakana characters based on template matching"
- "Stroke-number and stroke-order free on-line kanji character recognition"
- "Dynamic Positional Warping: Dynamic Time Warping for Online Handwriting"
- "Histograms of Oriented Gradients for Human Detection" (Dalal & Triggs, 2005)
