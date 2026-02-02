# Multiple Screenshot Capture - Current Limitation

## Issue
When a problem statement is too large and requires multiple screenshots:
- Each capture is processed independently
- No memory/combination of previous captures
- Results in incomplete or corrupted data on subsequent captures

## Current Behavior
1. **First capture**: Extracts partial problem (what's visible)
2. **Second capture**: Extracts different part, doesn't combine with first
3. **Third capture**: May have low quality, produces garbled output

## Workaround for Users
**Best Practice**: Capture the ENTIRE problem in ONE screenshot
- Zoom out browser to fit full problem
- Use full-screen mode
- Scroll to show most important parts (description + examples)
- Avoid capturing in multiple parts

## Future Enhancement (Not Implemented)
To support multi-capture combination, would need:
1. Session state tracking (problem_id)
2. Accumulation buffer for partial extractions
3. User signal for "capture complete"
4. Merge logic to combine JSON fragments
5. Timeout to clear accumulated state

This adds significant complexity and is not currently implemented.

## Recommendation
For now, focus on single high-quality captures that include:
- Problem title and description
- At least 1-2 examples with input/output
- Key constraints

Scout will extract what's visible and OSS-120B will generate solutions based on that.
