# Whisper API Integration Specification

## 1. Overview
This document specifies the robust question framing logic and integration with the Whisper model API. The system is designed to capture audio, detect speech, and transcribe it using a single API call per question, ensuring high performance (< 2s end-to-end) and reliability.

## 2. Architecture

### 2.1 Audio Capture & Processing
- **Module**: `src/modules/audio_processor.rs`
- **Input**: Raw PCM audio stream (Float32, 16kHz or resampled to 16kHz).
- **VAD (Voice Activity Detection)**: Energy-based thresholding with hysteresis.
  - `SPEECH_ENERGY_THRESHOLD`: 0.005 (RMS)
  - `SILENCE_TIMEOUT`: 800ms
  - `MIN_AUDIO_DURATION`: 500ms
  - `MAX_AUDIO_DURATION`: 120s

### 2.2 Question Framing Logic
The "Question Framing" ensures that only complete, valid utterances are sent to the API.
1. **Idle State**: Listens for energy > Threshold.
2. **Speech Detected**: Records start time. Accumulates audio samples.
3. **Buffering**: Continues accumulating until silence is detected for `SILENCE_TIMEOUT`.
4. **Validation**:
   - Checks total duration (must be > 500ms).
   - Checks signal quality (RMS > 0.001).
5. **Processing**:
   - Converts samples to WAV format (16-bit PCM, 16kHz, Mono).
   - **Single API Call**: A unique request is generated for the buffered audio.
   - **Concurrency Control**: New audio input is ignored/dropped while a request is in progress to prevent overlapping processing.

### 2.3 API Integration
- **Endpoint**: `https://api.groq.com/openai/v1/audio/transcriptions`
- **Model**: `whisper-large-v3`
- **Parameters**:
  - `language`: `en`
  - `temperature`: `0.0` (Deterministic)
  - `response_format`: `text`
  - `prompt`: "Technical interview question about programming, databases, algorithms, or computer science." (Context biasing)
- **Retry Mechanism**:
  - Max Retries: 3
  - Backoff: 500ms * retry_count
  - Triggers: Server errors (5xx), Network errors.

## 3. Performance Requirements
- **End-to-End Latency**: < 2 seconds (Audio End -> Transcription Result).
- **Audio Quality**: Handles background noise via energy thresholding.
- **Data Efficiency**: Sends only necessary WAV headers and PCM data.

## 4. Error Handling
- **Network Failures**: Automatic retry with exponential backoff.
- **API Errors**: Logged and reported to UI via WebSocket.
- **Audio Issues**: Low energy/short audio is discarded locally to save API calls.

## 5. Metrics & Logging
The system logs the following metrics for every request:
- `audio_duration_ms`: Length of the spoken question.
- `processing_time_ms`: Time taken by the Whisper API.
- `rms`: Root Mean Square amplitude of the audio.
- `total_time_ms`: End-to-end time including LLM response.
