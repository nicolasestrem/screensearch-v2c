//! Scoring: typed, DP-optimal boundary precision/recall/F1 (± tolerance), tool-recognition
//! accuracy, the parameter sweep, and the freeze-lookback stability analysis. Operates over
//! any [`crate::model::SessionSpan`] producer so PR4 can re-run its shipped segmenter through
//! it (the D9 referee contract). Filled in Task 8.
