# TTS Playback Changes - Direct Audio Playback

## Summary

Modified the `tts_notify_demo.rs` example to play audio directly using the `elevenlabs_rs::utils::play` function instead of saving to a file.

## Changes Made

### 1. Updated Imports

**Before:**
```rust
use std::fs::File;
use std::io::Write;
use bytes::Bytes;
```

**After:**
```rust
use elevenlabs_rs::utils::play;
// Removed file I/O imports
```

### 2. Modified Audio Handling

**Before:**
```rust
let audio_bytes = client.hit(endpoint).await
    .map_err(|e| format!("TTS error: {}", e))?;

// Save audio to file
let mut file = File::create("tts_output.mp3")
    .map_err(|e| format!("File create error: {}", e))?;
file.write_all(&audio_bytes)
    .map_err(|e| format!("File write error: {}", e))?;

Ok::<_, String>(text)
```

**After:**
```rust
let audio_bytes = client.hit(endpoint).await
    .map_err(|e| format!("TTS error: {}", e))?;

// Play audio directly
println!("  ♪ [TTS] Playing audio...");
play(audio_bytes)
    .map_err(|e| format!("Playback error: {}", e))?;

Ok::<_, String>(text)
```

### 3. Updated Success Messages

**Before:**
```rust
Ok(spoken_text) => {
    println!("  ✓ [TTS] Audio saved to tts_output.mp3");
    println!("  ✓ [TTS] Notification delivered\n");
    Ok(ToolResult::text(format!("Spoke: \"{}\" (saved to tts_output.mp3)", spoken_text)))
}
```

**After:**
```rust
Ok(spoken_text) => {
    println!("  ✓ [TTS] Playback complete");
    println!("  ✓ [TTS] Notification delivered\n");
    Ok(ToolResult::text(format!("Spoke: \"{}\"", spoken_text)))
}
```

### 4. Updated Documentation

**Before:**
```
//! Requirements:
//! - Set ELEVENLABS_API_KEY environment variable
//! - Audio file will be saved to tts_output.mp3
```

**After:**
```
//! Requirements:
//! - Set ELEVENLABS_API_KEY environment variable
//! - Audio will be played directly (no file saved)
```

## How It Works

The `elevenlabs_rs::utils::play` function uses the `rodio` audio library to:
1. Create an audio output stream
2. Decode the MP3 audio data
3. Play it through the default audio device
4. Block until playback completes

### Function Implementation (from elevenlabs_rs)

```rust
pub fn play(data: Bytes) -> Result<()> {
    let (_stream, stream_handle) = OutputStream::try_default()?;
    let source = Decoder::new(std::io::Cursor::new(data))?;
    let sink = Sink::try_new(&stream_handle)?;
    sink.append(source);
    sink.sleep_until_end();
    Ok(())
}
```

## Benefits

1. **No File I/O** - Audio plays immediately without disk write
2. **Simpler Code** - Less error handling for file operations
3. **Cleaner** - No leftover files to manage
4. **Faster** - Direct playback without intermediate file step
5. **Better UX** - Instant audio feedback

## Usage

```bash
# Run the example (with valid API key)
ELEVENLABS_API_KEY=your_key cargo run --example tts_notify_demo
```

### Expected Output

```
=== TTS Notification Demo ===

This demo uses ElevenLabs API for real text-to-speech.

Starting task: List files in current directory

[... MCP initialization ...]

  🔊 [TTS] Speaking: "Task complete"
  ♪ [TTS] Playing audio...
  ✓ [TTS] Playback complete
  ✓ [TTS] Notification delivered

[... task completion ...]

=== Demo Complete ===

Demonstrated:
✓ Custom TTS tool via SDK MCP server
✓ Real ElevenLabs API integration
✓ Hook on conversation Stop event
✓ Voice notification when task completes
✓ Audio played directly using rodio
```

## Dependencies

The audio playback relies on:
- `elevenlabs_rs` (dev dependency) - provides the `play` function
- `rodio` (transitive dependency via elevenlabs_rs) - audio playback library
- `bytes` (transitive dependency via elevenlabs_rs) - byte buffer type

No additional dependencies needed in `Cargo.toml`.

## Notes

- Audio is played through the system's default audio output device
- Playback blocks until audio completes (synchronous)
- Requires a valid `ELEVENLABS_API_KEY` environment variable
- Uses ElevenLabs' Brian voice with the MultilingualV2 model
