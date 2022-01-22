use glib::{clone, closure};
use gtk::{gdk, gio, glib, prelude::*, subclass::prelude::*, CompositeTemplate};
use tdgrand::{enums::MessageContent, types::File};

use crate::session::chat::{BoxedMessageContent, Message};
use crate::session::content::{message_row::MessageMediaContent, MessageRow, MessageRowExt};
use crate::utils::parse_formatted_text;
use crate::Session;

mod imp {
    use super::*;
    use glib::WeakRef;
    use std::cell::RefCell;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/com/github/melix99/telegrand/ui/content-message-media.ui")]
    pub struct MessageMedia {
        pub binding: RefCell<Option<gtk::ExpressionWatch>>,
        pub handler_id: RefCell<Option<glib::SignalHandlerId>>,
        pub old_message: WeakRef<glib::Object>,
        #[template_child]
        pub content: TemplateChild<MessageMediaContent>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MessageMedia {
        const NAME: &'static str = "ContentMessageMedia";
        type Type = super::MessageMedia;
        type ParentType = MessageRow;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for MessageMedia {
        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);
            obj.connect_message_notify(|obj, _| obj.update_widget());
        }
    }

    impl WidgetImpl for MessageMedia {}
}

glib::wrapper! {
    pub struct MessageMedia(ObjectSubclass<imp::MessageMedia>)
        @extends gtk::Widget, MessageRow;
}

impl MessageMedia {
    fn update_widget(&self) {
        let imp = self.imp();

        if let Some(old_message) = imp.old_message.upgrade() {
            old_message.disconnect(imp.handler_id.take().unwrap());
            imp.binding.take().unwrap().unwatch();
        }

        if let Some(message) = self.message() {
            let message = message.downcast_ref::<Message>().unwrap();

            // Setup caption expression
            let caption_binding = Message::this_expression("content")
                .chain_closure::<String>(closure!(|_: Message, content: BoxedMessageContent| {
                    parse_formatted_text(match content.0 {
                        MessageContent::MessageAnimation(data) => data.caption,
                        MessageContent::MessagePhoto(data) => data.caption,
                        _ => unreachable!(),
                    })
                }))
                .bind(&*imp.content, "caption", Some(message));
            imp.binding.replace(Some(caption_binding));

            // Load media
            let handler_id =
                message.connect_content_notify(clone!(@weak self as obj => move |message, _| {
                    obj.update_media(message);
                }));
            imp.handler_id.replace(Some(handler_id));
            self.update_media(message);
        }

        imp.old_message.set(self.message().as_ref());
    }

    fn update_media(&self, message: &Message) {
        let imp = self.imp();

        imp.content.set_paintable(None);

        let message_content = message.content().0;
        let file = match message_content {
            MessageContent::MessageAnimation(ref data) => {
                imp.content
                    .set_aspect_ratio(data.animation.width as f64 / data.animation.height as f64);

                &data.animation.animation
            }
            MessageContent::MessagePhoto(ref data) => {
                let photo_size = data.photo.sizes.last().unwrap();
                imp.content
                    .set_aspect_ratio(photo_size.width as f64 / photo_size.height as f64);
                &photo_size.photo
            }
            _ => unreachable!(),
        };

        if file.local.is_downloading_completed {
            imp.content.set_download_progress(1.0);
            self.load_media_from_path(&file.local.path);
        } else {
            imp.content.set_download_progress(0.0);
            self.download_media(file.id, &message.chat().session());
        }
    }

    fn download_media(&self, file_id: i32, session: &Session) {
        let (sender, receiver) = glib::MainContext::sync_channel::<File>(Default::default(), 5);

        receiver.attach(
            None,
            clone!(@weak self as obj => @default-return glib::Continue(false), move |file| {
                if file.local.is_downloading_completed {
                    obj.imp().content.set_download_progress(1.0);
                    obj.load_media_from_path(&file.local.path);
                } else {
                    let progress = file.local.downloaded_size as f64 / file.expected_size as f64;
                    obj.imp().content.set_download_progress(progress);
                }

                glib::Continue(true)
            }),
        );

        session.download_file(file_id, sender);
    }

    fn load_media_from_path(&self, path: &str) {
        if let Some(message) = self.message() {
            let message = message.downcast_ref::<Message>().unwrap();
            self.imp().content.set_paintable(match message.content().0 {
                MessageContent::MessageAnimation(_) => {
                    let media_file = gtk::MediaFile::for_filename(&path);
                    media_file.set_loop(true);
                    media_file.play();
                    Some(media_file.upcast())
                }
                MessageContent::MessagePhoto(_) => {
                    // TODO: Consider changing this to use an async api when
                    // https://github.com/gtk-rs/gtk4-rs/pull/777 is merged
                    let file = gio::File::for_path(path);
                    Some(gdk::Texture::from_file(&file).unwrap().upcast())
                }
                _ => unreachable!(),
            });
        }
    }
}
