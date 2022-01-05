use gettextrs::gettext;
use gtk::{glib, prelude::*, subclass::prelude::*, CompositeTemplate};

use crate::session::chat::{Message, SponsoredMessage};

mod imp {
    use super::*;
    use once_cell::sync::Lazy;
    use std::cell::RefCell;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/com/github/melix99/telegrand/ui/content-message-indicators.ui")]
    pub struct MessageIndicators {
        pub message: RefCell<Option<glib::Object>>,
        #[template_child]
        pub timestamp: TemplateChild<gtk::Label>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MessageIndicators {
        const NAME: &'static str = "ContentMessageIndicators";
        type Type = super::MessageIndicators;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for MessageIndicators {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![glib::ParamSpec::new_object(
                    "message",
                    "Message",
                    "The message relative to this indicators",
                    glib::Object::static_type(),
                    glib::ParamFlags::READWRITE | glib::ParamFlags::EXPLICIT_NOTIFY,
                )]
            });
            PROPERTIES.as_ref()
        }

        fn set_property(
            &self,
            obj: &Self::Type,
            _id: usize,
            value: &glib::Value,
            pspec: &glib::ParamSpec,
        ) {
            match pspec.name() {
                "message" => obj.set_message(value.get().unwrap()),
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "message" => obj.message().to_value(),
                _ => unimplemented!(),
            }
        }

        fn dispose(&self, _obj: &Self::Type) {
            self.timestamp.unparent();
        }
    }

    impl WidgetImpl for MessageIndicators {}
}

glib::wrapper! {
    pub struct MessageIndicators(ObjectSubclass<imp::MessageIndicators>)
        @extends gtk::Widget;
}

impl Default for MessageIndicators {
    fn default() -> Self {
        Self::new()
    }
}

impl MessageIndicators {
    pub fn new() -> Self {
        glib::Object::new(&[]).expect("Failed to create MessageIndicators")
    }

    pub fn message(&self) -> Option<glib::Object> {
        let self_ = imp::MessageIndicators::from_instance(self);
        self_.message.borrow().to_owned()
    }

    pub fn set_message(&self, message: Option<glib::Object>) {
        if self.message() == message {
            return;
        }

        let self_ = imp::MessageIndicators::from_instance(self);
        if let Some(ref message) = message {
            if let Some(message) = message.downcast_ref::<Message>() {
                self_.timestamp.set_visible(true);
                self_.timestamp.set_label(&timestamp_from_message(message));
            } else if message.downcast_ref::<SponsoredMessage>().is_some() {
                self_.timestamp.set_visible(false);
            } else {
                unreachable!("Unexpected message type: {:?}", message);
            }
        } else {
            self_.timestamp.set_visible(false);
        }

        self_.message.replace(message);
        self.notify("message");
    }
}

fn timestamp_from_message(message: &Message) -> String {
    let datetime = glib::DateTime::from_unix_utc(message.date() as i64)
        .and_then(|t| t.to_local())
        .unwrap();
    // Translators: This is a time format for the message timestamp without seconds
    datetime.format(&gettext("%l:%M %p")).unwrap().to_string()
}
