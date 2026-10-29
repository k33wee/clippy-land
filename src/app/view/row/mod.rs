mod render;
mod state;

/// Keep text, images, and keyboard targets on the same compact row rhythm.
pub(in crate::app) const HISTORY_ROW_HEIGHT: f32 = 112.0;

pub(in crate::app) use render::history_row;
pub(in crate::app) use state::{RowContent, RowRenderState};
