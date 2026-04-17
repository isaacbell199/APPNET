# Task ID: 2-a - Frontend Bug Fix Agent

## Summary
Fixed 6 HIGH severity bugs in the frontend code of the Tauri + Next.js desktop application.

## Bugs Fixed

### BUG #9: store.ts - Models persisted incorrectly
**File**: `src/lib/store.ts`
**Problem**: Models were persisted in localStorage but should be loaded dynamically from the backend.
**Fix**: Removed `models` from the `partialize` function. Added a comment explaining models are fetched from the backend.

### BUG #10: editor-view.tsx - autoSaveTimerRef not cleaned up
**File**: `src/components/views/editor-view.tsx`
**Problem**: The autoSaveTimerRef.current setTimeout was not cleaned up when the component unmounts.
**Fix**: Added a dedicated useEffect cleanup that clears the timer on unmount.

### BUG #11: models-view.tsx - duoModelSystemPrompt unused (dead code)
**File**: `src/components/views/models-view.tsx`
**Problem**: `duoModelSystemPrompt` state variable was declared but never used.
**Fix**: Removed the unused variable `duoModelSystemPrompt` and `setDuoModelSystemPrompt`, along with the corresponding UI section.

### BUG #12: floating-ai-tools.tsx - setTimeout for unsubscribe
**File**: `src/components/floating-ai-tools.tsx`
**Problem**: setTimeout was used to unsubscribe from events after 30-60 seconds, causing potential memory leaks.
**Fix**: Used useRef to store the unsubscribe function and timeout ID, with cleanup in useEffect return.

### BUG #19: models-view.tsx - GPU layers not synchronized
**File**: `src/components/views/models-view.tsx`
**Problem**: When loading a model, the local `gpuLayers` state may not match what's actually used.
**Fix**: Added getLLMSettings/updateLLMSettings calls before tauriLoadModel to ensure GPU settings are passed correctly.

### BUG #23: models-view.tsx - Error stack not displayed
**File**: `src/components/views/models-view.tsx`
**Problem**: The error stack was available in errorState.stack but not shown to the user.
**Fix**: Added a collapsible section in the error alert to show the stack trace for debugging.

## Files Modified
- `src/lib/store.ts`
- `src/components/views/editor-view.tsx`
- `src/components/views/models-view.tsx`
- `src/components/floating-ai-tools.tsx`

## Verification
- ESLint passed with no errors
- All changes are minimal and targeted
- Preserved all existing functionality
