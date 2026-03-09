//! Macros for dispatching raw boxes to typed views.

/// Dispatch a raw box to typed view handlers based on its four-cc.
///
/// Each arm names a `TypedBoxView` implementor and a binding variable.
/// The macro matches the raw box's type code against each view's
/// [`BOX_TYPE`](crate::container::TypedBoxView::BOX_TYPE), parses it,
/// and runs the corresponding body.
///
/// The `Err(e)` arm handles parse errors for matched types. Use `Err(_)`
/// to explicitly discard errors.
///
/// # Example
///
/// ```
/// use isobmff_syntax::{dispatch_box, RawBox};
/// use isobmff_syntax::boxes::{
///     MovieHeaderBox, MovieHeaderBoxView,
///     TrackHeaderBox, TrackHeaderBoxView,
/// };
///
/// fn handle(raw_box: &RawBox<'_>) -> String {
///     dispatch_box!(raw_box, {
///         MovieHeaderBoxView(view) => {
///             format!("timescale: {}", view.timescale())
///         },
///         TrackHeaderBoxView(view) => {
///             format!("track_id: {}", view.track_id())
///         },
///         _ => {
///             format!("unknown: {}", raw_box.box_type())
///         },
///         Err(_) => {
///             format!("parse error")
///         },
///     })
/// }
/// ```
#[macro_export]
macro_rules! dispatch_box {
    ($raw:expr, {
        $( $($View:ident)::+ ( $binding:ident ) => $body:expr ),+,
        _ => $default:expr,
        Err( $err:pat ) => $err_body:expr $(,)?
    }) => {
        match $raw.box_type() {
            $( <$($View)::+ as $crate::container::TypedBoxView>::BOX_TYPE => {
                match <$($View)::+ as $crate::container::TypedBoxView>::from_raw_box($raw.data()) {
                    Ok($binding) => $body,
                    Err($err) => $err_body,
                }
            } )+
            _ => $default,
        }
    };
}
