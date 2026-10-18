//! A one-child container that keeps its child 16:9 (height follows width), with optional
//! maximum width and height. Used for the video so the player card keeps the picture's shape
//! inside scrolling pages, where GtkAspectFrame has no height to work with.

use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

mod imp {
    use super::*;
    use std::cell::{Cell, RefCell};

    pub struct RatioBox {
        pub child: RefCell<Option<gtk::Widget>>,
        pub ratio: Cell<f64>,
        pub max_w: Cell<i32>,
        pub max_h: Cell<i32>,
    }

    impl Default for RatioBox {
        fn default() -> Self {
            RatioBox {
                child: RefCell::new(None),
                ratio: Cell::new(16.0 / 9.0),
                max_w: Cell::new(0),
                max_h: Cell::new(0),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for RatioBox {
        const NAME: &'static str = "MadhyamRatioBox";
        type Type = super::RatioBox;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("ratiobox");
        }
    }

    impl ObjectImpl for RatioBox {
        fn dispose(&self) {
            if let Some(c) = self.child.take() {
                c.unparent();
            }
        }
    }

    impl WidgetImpl for RatioBox {
        fn request_mode(&self) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::ConstantSize
        }

        /// Natural size: the largest 16:9 box inside the limits (the App recomputes them from
        /// the window size). The minimum stays small so the window can always shrink.
        fn measure(&self, orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
            let ratio = self.ratio.get();
            let (max_w, max_h) = (self.max_w.get(), self.max_h.get());
            let mut w = if max_w > 0 { max_w as f64 } else { 640.0 };
            if max_h > 0 {
                w = w.min(max_h as f64 * ratio);
            }
            let w = w.max(240.0);
            if orientation == gtk::Orientation::Horizontal {
                (240, w.round() as i32, -1, -1)
            } else {
                // The height is required, not just preferred: a scrolling page shares spare
                // height between its children and would squash the picture otherwise.
                let h = (w / ratio).round() as i32;
                (h, h, -1, -1)
            }
        }

        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            if let Some(c) = self.child.borrow().as_ref() {
                c.allocate(width, height, baseline, None);
            }
        }
    }
}

glib::wrapper! {
    pub struct RatioBox(ObjectSubclass<imp::RatioBox>) @extends gtk::Widget, @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for RatioBox {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl RatioBox {
    pub fn set_child(&self, child: Option<&impl IsA<gtk::Widget>>) {
        let imp = self.imp();
        if let Some(old) = imp.child.take() {
            old.unparent();
        }
        if let Some(c) = child {
            c.set_parent(self);
            *imp.child.borrow_mut() = Some(c.clone().upcast());
        }
        self.queue_resize();
    }

    pub fn child(&self) -> Option<gtk::Widget> {
        self.imp().child.borrow().clone()
    }

    /// Limits in pixels (0 = none).
    pub fn set_limits(&self, max_w: i32, max_h: i32) {
        let imp = self.imp();
        if imp.max_w.get() != max_w || imp.max_h.get() != max_h {
            imp.max_w.set(max_w);
            imp.max_h.set(max_h);
            self.queue_resize();
        }
    }
}
