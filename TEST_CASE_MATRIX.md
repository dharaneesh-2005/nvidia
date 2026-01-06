# Test Case Validation Matrix

| ID | Test Scenario | Description | Expected Outcome | Status |
|----|---------------|-------------|------------------|--------|
| **TC01** | **Normal Question** | Clear speech, 5 seconds duration, "What is a binary tree?" | Transcription: "What is a binary tree?" <br> Latency: < 2s | ✅ Implemented |
| **TC02** | **Short Utterance** | Speech < 0.5s (e.g., cough, tap) | Audio discarded. No API call. | ✅ Implemented |
| **TC03** | **Long Speech** | Continuous speech for 2 minutes | Force transcription at 120s mark. | ✅ Implemented |
| **TC04** | **Silence Gap** | Speech with 500ms pause (below 800ms timeout) | Audio accumulated as single question. | ✅ Implemented |
| **TC05** | **Silence Timeout** | Speech followed by 1s silence | Transcription triggers after 800ms silence. | ✅ Implemented |
| **TC06** | **Low Volume** | Whisper/Quiet speech (Energy > Threshold but low) | Transcribed correctly (Whisper is robust). | ✅ Implemented |
| **TC07** | **Background Noise** | Constant low hum (Energy < Threshold) | Ignored. No speech detected. | ✅ Implemented |
| **TC08** | **API Failure** | Network disconnect during call | Retry 3 times, then report error. | ✅ Implemented |
| **TC09** | **Rapid Questions** | Question 1 ... Question 2 immediately | Q1 processed. Q2 ignored if Q1 is still processing (Single Call Logic). | ✅ Implemented |
| **TC10** | **Context Bias** | Technical jargon "SQL injection" | Prompt ensures correct spelling vs generic words. | ✅ Implemented |
