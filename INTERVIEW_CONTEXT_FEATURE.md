# Interview Context Enhancement Feature

## Overview
This feature adds **dual-stream audio capture** to provide better context-aware AI responses during interviews. The system now captures both the interviewer's questions AND the candidate's answers, allowing the AI to provide more relevant and contextual suggestions.

## Architecture

### Two Parallel Audio Streams

```
┌─────────────────────────────────────────────────────────────┐
│                    AUDIO CAPTURE LAYER                       │
├──────────────────────────────┬──────────────────────────────┤
│   Stream 1: Screen Audio     │   Stream 2: Microphone       │
│   (Interviewer Questions)    │   (Candidate Answers)        │
│                              │                              │
│   WASAPI Loopback           │   cpal Input Device          │
│   ↓                         │   ↓                          │
│   AudioProcessor            │   MicProcessor               │
│   ↓                         │   ↓                          │
│   Whisper Transcription     │   Whisper Transcription      │
│   ↓                         │   ↓                          │
│   Question Context          │   Answer Context             │
│   (Conversation History)    │   (Last 10 messages)         │
└──────────────────────────────┴──────────────────────────────┘
                              ↓
                    When Question Detected
                              ↓
                ┌─────────────────────────┐
                │  Combine Both Contexts  │
                │  Question + Answer      │
                └─────────────────────────┘
                              ↓
                         Groq AI
                              ↓
                   Context-Aware Response
```

## New Components

### 1. **MicCapture** (`src/modules/mic_capture.rs`)
- Captures audio from default microphone input device
- Mirrors `audio.rs` but for INPUT device instead of OUTPUT
- Downsamples to 16kHz for Whisper
- Converts to mono
- Sends processed audio chunks via crossbeam channel

### 2. **MicProcessor** (`src/modules/mic_processor.rs`)
- Processes microphone audio chunks continuously
- Transcribes using Whisper API
- Maintains rolling context window (last 10 messages)
- Voice Activity Detection (VAD) to filter silence
- No AI calls - just transcription and storage

### 3. **Candidate Context Window** (`main.rs`)
- New type: `CandidateContext = Arc<RwLock<VecDeque<String>>>`
- Stores last 10 candidate transcriptions
- Thread-safe, shared across components
- Automatically maintains size limit

### 4. **Enhanced Groq Client** (`src/modules/groq.rs`)
- New method: `chat_with_context()`
- Accepts candidate context in addition to conversation history
- Updates system prompt to include candidate's recent responses
- AI can now reference what the candidate already said

## How It Works

### Continuous Operation
1. **Microphone Always On**: Starts capturing when app launches
2. **Continuous Transcription**: Processes audio in 5-second chunks
3. **Context Storage**: Stores transcriptions in rolling window
4. **No Manual Control**: Fully automatic, no user interaction needed

### When Question is Asked
1. Interviewer asks question (captured via screen audio)
2. AudioProcessor detects question and transcribes it
3. AudioProcessor reads candidate context window
4. Both contexts sent to Groq AI:
   - Question: "Explain microservices"
   - Candidate Recent: ["I mentioned distributed systems...", "APIs are important..."]
5. AI generates context-aware response
6. Response displayed to user

### Example Flow

```
Interviewer: "Tell me about microservices architecture"
[Screen audio captures this]

You: "Well, microservices are independent services that communicate via APIs..."
[Mic captures this → transcribed → stored in candidate context]

System combines:
- Question: "Tell me about microservices architecture"
- Your Recent Answer: "microservices are independent services that communicate via APIs"

AI Response: "Great start! You can also mention:
- Service discovery (Eureka, Consul)
- API Gateway pattern
- Database per service
- Event-driven communication with message queues"
```

## Key Features

✅ **Always Listening**: Microphone captures continuously  
✅ **Automatic Transcription**: No manual triggers needed  
✅ **Context-Aware AI**: AI knows what you said  
✅ **Rolling Window**: Last 10 messages kept  
✅ **Voice Activity Detection**: Filters out silence  
✅ **Parallel Processing**: Both streams independent  
✅ **No UI Clutter**: Works silently in background  

## Technical Details

### Voice Activity Detection
- RMS energy calculation
- Threshold: 0.01 (adjustable)
- Filters silence to save API calls
- Only transcribes meaningful speech

### Context Window Management
```rust
// Candidate context (last 10)
candidate_context: VecDeque<String>

// When new transcription arrives:
context.push_back(new_transcription);
if context.len() > 10 {
    context.pop_front(); // Remove oldest
}
```

### AI Prompt Enhancement
```
System: You are helping a CS student answer interview questions.

CANDIDATE'S RECENT RESPONSES:
1. microservices are independent services
2. APIs are important for communication
3. distributed systems need coordination

Current Question: {question}

Task: Build on what the candidate said and provide additional points.
```

## Files Modified

### New Files
- `src/modules/mic_capture.rs` - Microphone capture
- `src/modules/mic_processor.rs` - Mic audio processor
- `INTERVIEW_CONTEXT_FEATURE.md` - This documentation

### Modified Files
- `src/modules/mod.rs` - Added new modules
- `src/main.rs` - Added candidate context, mic capture initialization
- `src/modules/audio_processor.rs` - Updated to use candidate context
- `src/modules/groq.rs` - Added `chat_with_context()` method

## Configuration

### Audio Settings
- **Sample Rate**: 16kHz (Whisper requirement)
- **Chunk Duration**: 5 seconds
- **Context Size**: 10 messages
- **VAD Threshold**: 0.01 RMS energy

### API Usage
- **Whisper Model**: whisper-large-v3
- **Text Model**: llama-3.3-70b-versatile
- **Transcription**: Per 5-second chunk
- **AI Response**: Per question

## Benefits

### For Candidates
- AI understands your context
- Better follow-up suggestions
- More relevant responses
- Natural conversation flow

### For Accuracy
- Reduces misunderstandings
- AI knows what you already covered
- Can suggest what you missed
- Contextual depth

## Limitations

1. **Microphone Required**: Needs working mic input
2. **API Costs**: More Whisper API calls (continuous transcription)
3. **Privacy**: Always listening (no visual indicator per requirements)
4. **Context Size**: Limited to last 10 messages
5. **Language**: English only (Whisper limitation)

## Future Enhancements

- [ ] Configurable context window size
- [ ] Speaker diarization (distinguish multiple voices)
- [ ] Sentiment analysis on candidate responses
- [ ] Real-time transcription display (optional)
- [ ] Context export/save feature
- [ ] Multi-language support

## Testing

### Verify Microphone Capture
1. Start the application
2. Check console for: `✅ Using microphone: [device name]`
3. Speak into microphone
4. Check console for: `[Candidate] Transcribed: [your speech]`

### Verify Context Integration
1. Have an interview conversation
2. Speak your answer
3. Wait for interviewer's next question
4. Check AI response references your previous answer

### Console Output
```
=== MICROPHONE CAPTURE MODE ===
✅ Using microphone: Microphone (Realtek Audio)
   This will capture candidate's voice continuously
   Native format: 48000Hz, 2 channels
   Will downsample to: 16000Hz, 1 channel (mono) for Whisper
✅ Microphone active - capturing candidate's voice

[Candidate] Transcribed: microservices are independent services
[Candidate] Context size: 1/10

[Candidate] Transcribed: they communicate via APIs
[Candidate] Context size: 2/10
```

## Troubleshooting

### Microphone Not Working
- Check default input device in Windows Sound settings
- Ensure microphone permissions granted
- Verify mic is not muted

### No Transcriptions
- Check console for errors
- Verify Groq API key is valid
- Ensure speaking loud enough (VAD threshold)

### Context Not Used
- Verify candidate context is populated (check console)
- Ensure `chat_with_context()` is being called
- Check AI prompt includes candidate context

## Summary

This feature transforms the interview helper from a simple Q&A tool into a context-aware assistant that understands the full conversation. By capturing both the interviewer's questions and the candidate's answers, the AI can provide much more relevant and helpful suggestions.

**Key Innovation**: Dual-stream audio processing with automatic context management, requiring zero user interaction while providing maximum value.
