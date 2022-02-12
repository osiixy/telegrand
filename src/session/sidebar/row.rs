use gettextrs::gettext;
use glib::closure;
use gtk::{glib, prelude::*, subclass::prelude::*, CompositeTemplate};
use std::borrow::Cow;
use tdgrand::enums::{CallDiscardReason, InputMessageContent, MessageContent};
use tdgrand::types::{DraftMessage, MessageCall};

use crate::session::chat::{
    BoxedChatNotificationSettings, BoxedDraftMessage, BoxedMessageContent, Message, MessageSender,
};
use crate::session::sidebar::Avatar;
use crate::session::{BoxedScopeNotificationSettings, Chat, ChatType, Session, User};
use crate::utils::{dim_and_escape, escape, human_friendly_duration};

mod imp {
    use super::*;
    use once_cell::sync::Lazy;
    use std::cell::RefCell;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/com/github/melix99/telegrand/ui/sidebar-row.ui")]
    pub struct Row {
        /// A `Chat` or `User`
        pub item: RefCell<Option<glib::Object>>,
        pub bindings: RefCell<Vec<gtk::ExpressionWatch>>,
        #[template_child]
        pub avatar: TemplateChild<Avatar>,
        #[template_child]
        pub main_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub bottom_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub title_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub timestamp_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub message_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub pin_icon: TemplateChild<gtk::Image>,
        #[template_child]
        pub unread_mention_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub unread_count_label: TemplateChild<gtk::Label>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Row {
        const NAME: &'static str = "SidebarRow";
        type Type = super::Row;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Row {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![glib::ParamSpecObject::new(
                    "item",
                    "Item",
                    "The item of this row",
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
                "item" => obj.set_item(value.get().unwrap()),
                _ => unimplemented!(),
            }
        }

        fn property(&self, obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "item" => obj.item().to_value(),
                _ => unimplemented!(),
            }
        }

        fn dispose(&self, _obj: &Self::Type) {
            self.avatar.unparent();
            self.main_box.unparent();
        }
    }

    impl WidgetImpl for Row {}
}

glib::wrapper! {
    pub struct Row(ObjectSubclass<imp::Row>)
        @extends gtk::Widget;
}

impl Default for Row {
    fn default() -> Self {
        Self::new()
    }
}

impl Row {
    pub fn new() -> Self {
        glib::Object::new(&[]).expect("Failed to create Row")
    }

    pub fn item(&self) -> Option<glib::Object> {
        self.imp().item.borrow().to_owned()
    }

    pub fn set_item(&self, item: Option<glib::Object>) {
        if self.item() == item {
            return;
        }

        let imp = self.imp();
        let mut bindings = imp.bindings.borrow_mut();

        while let Some(binding) = bindings.pop() {
            binding.unwatch();
        }

        if let Some(ref item) = item {
            if let Some(chat) = item.downcast_ref::<Chat>() {
                imp.timestamp_label.set_visible(true);
                imp.bottom_box.set_visible(true);

                let last_message_expression = Chat::this_expression("last-message");
                let draft_message_expression = Chat::this_expression("draft-message");
                let unread_mention_count_expression = Chat::this_expression("unread-mention-count");
                let unread_count_expression = Chat::this_expression("unread-count");
                let is_pinned_expression = Chat::this_expression("is-pinned");
                let notification_settings_expression =
                    Chat::this_expression("notification-settings");
                let session_expression = Chat::this_expression("session");

                // Title label bindings
                let title_binding =
                    Chat::this_expression("title").bind(&*imp.title_label, "label", Some(chat));
                bindings.push(title_binding);

                // Timestamp label bindings
                let timestamp_binding = gtk::ClosureExpression::new::<i32, _, _>(
                    &[
                        draft_message_expression.clone().upcast(),
                        last_message_expression.clone().upcast(),
                    ],
                    closure!(|_: Chat,
                              draft_message: Option<BoxedDraftMessage>,
                              last_message: Option<Message>| {
                        draft_message.map(|m| m.0.date).unwrap_or_else(|| {
                            // ... Or, if there is no draft message use the timestamp of the
                            // last message.
                            last_message
                                // TODO: Sometimes just unwrapping here crashes because the
                                // update hasn't yet arrived. For the future, I think we could
                                // set the last message early in chat construction to remove
                                // this workaround.
                                .map(|m| m.date())
                                .unwrap_or_default()
                        })
                    }),
                )
                .chain_closure::<glib::GString>(closure!(|_: Chat, date: i32| {
                    let datetime_now = glib::DateTime::now_local().unwrap();
                    let datetime = glib::DateTime::from_unix_utc(date as i64)
                        .and_then(|t| t.to_local())
                        .unwrap();

                    let difference = datetime_now.difference(&datetime);
                    let hours_difference = difference.as_hours();
                    let days_difference = difference.as_days();

                    if hours_difference <= 16 {
                        // Show the time
                        // Translators: This is a time format for the chat list without seconds
                        datetime.format(&gettext("%l:%M %p")).unwrap()
                    } else if days_difference < 6 {
                        // Show the day of the week
                        datetime.format("%a").unwrap()
                    } else if days_difference < 364 {
                        // Show the day and the month
                        datetime.format("%d %b").unwrap()
                    } else {
                        // Show the entire date
                        datetime.format("%x").unwrap()
                    }
                }))
                .bind(&*imp.timestamp_label, "label", Some(chat));
                bindings.push(timestamp_binding);

                // Last message and draft message label bindings
                let content_expression =
                    last_message_expression.chain_property::<Message>("content");
                // FIXME: the sender name should be part of this expression.
                let message_binding = gtk::ClosureExpression::new::<String, _, _>(
                    &[
                        draft_message_expression.upcast(),
                        last_message_expression.upcast(),
                        content_expression.upcast(),
                    ],
                    closure!(|_: Chat,
                              draft_message: Option<BoxedDraftMessage>,
                              last_message: Option<Message>,
                              _content: BoxedMessageContent| {
                        // Either, if there is a draft message, retrieve the content from it. ...
                        draft_message
                            .map(|message| {
                                format!(
                                    "<span foreground=\"#e01b24\">{}:</span> {}",
                                    gettext("Draft"),
                                    stringify_draft_message(&message.0)
                                )
                            })
                            .unwrap_or_else(|| {
                                // ... Or, if there is no draft message use the content of the
                                // last message.
                                last_message
                                    // TODO: Sometimes just unwrapping here crashes because the
                                    // update hasn't yet arrived. For the future, I think we could
                                    // set the last message early in chat construction to remove
                                    // this workaround.
                                    .map(stringify_message)
                                    .unwrap_or_default()
                            })
                    }),
                )
                .bind(&*imp.message_label, "label", Some(chat));
                bindings.push(message_binding);

                // Unread mention visibility binding
                let unread_mention_binding = unread_mention_count_expression
                    .chain_closure::<bool>(closure!(|_: Chat, unread_mention_count: i32| {
                        unread_mention_count > 0
                    }))
                    .bind(&*imp.unread_mention_label, "visible", Some(chat));
                bindings.push(unread_mention_binding);

                // Unread count binding
                let unread_binding =
                    unread_count_expression.bind(&*imp.unread_count_label, "label", Some(chat));
                bindings.push(unread_binding);

                // Unread count visibility binding
                let unread_binding = gtk::ClosureExpression::new::<bool, _, _>(
                    &[
                        unread_count_expression.clone().upcast(),
                        unread_mention_count_expression.upcast(),
                    ],
                    closure!(|_: Chat, unread_count: i32, unread_mention_count: i32| {
                        unread_count > 0 && (unread_mention_count != 1 || unread_count > 1)
                    }),
                )
                .bind(&*imp.unread_count_label, "visible", Some(chat));
                bindings.push(unread_binding);

                // Unread count css classes binding
                let scope_notification_settings_expression = session_expression
                    .chain_property::<Session>(match chat.type_() {
                        ChatType::Private(_) | ChatType::Secret(_) => {
                            "private-chats-notification-settings"
                        }
                        ChatType::BasicGroup(_) => "group-chats-notification-settings",
                        ChatType::Supergroup(supergroup) => {
                            if supergroup.is_channel() {
                                "channel-chats-notification-settings"
                            } else {
                                "group-chats-notification-settings"
                            }
                        }
                    });
                let unread_binding = gtk::ClosureExpression::new::<Vec<String>, _, _>(
                    &[
                        notification_settings_expression.upcast(),
                        scope_notification_settings_expression.upcast(),
                    ],
                    closure!(|_: Chat,
                              notification_settings: BoxedChatNotificationSettings,
                              scope_notification_settings: BoxedScopeNotificationSettings| {
                        vec![
                            "unread-count".to_string(),
                            if notification_settings.0.use_default_mute_for {
                                if scope_notification_settings.0
                                    .map(|s| s.mute_for > 0)
                                    .unwrap_or(notification_settings.0.mute_for > 0)
                                {
                                    "unread-count-muted"
                                } else {
                                    "unread-count-unmuted"
                                }
                            } else if notification_settings.0.mute_for > 0 {
                                "unread-count-muted"
                            } else {
                                "unread-count-unmuted"
                            }
                            .to_string(),
                        ]
                    }),
                )
                .bind(&*imp.unread_count_label, "css-classes", Some(chat));
                bindings.push(unread_binding);

                // Pin icon visibility binding
                let pin_binding = gtk::ClosureExpression::new::<bool, _, _>(
                    &[
                        is_pinned_expression.upcast(),
                        unread_count_expression.upcast(),
                    ],
                    closure!(|_: Chat, is_pinned: bool, unread_count: i32| {
                        is_pinned && unread_count <= 0
                    }),
                )
                .bind(&*imp.pin_icon, "visible", Some(chat));
                bindings.push(pin_binding);
            } else if let Some(user) = item.downcast_ref::<User>() {
                imp.timestamp_label.set_visible(false);
                imp.bottom_box.set_visible(false);

                // Title label binding
                let user_expression = gtk::ConstantExpression::new(user);
                let title_binding = User::full_name_expression(&user_expression).bind(
                    &*imp.title_label,
                    "label",
                    glib::Object::NONE,
                );
                bindings.push(title_binding);
            } else {
                unreachable!("Unexpected item type: {:?}", item);
            }
        }

        imp.item.replace(item);
        self.notify("item");
    }
}

fn stringify_message(message: Message) -> String {
    let mut show_sender = match message.chat().type_() {
        ChatType::BasicGroup(_) => true,
        ChatType::Supergroup(supergroup) => !supergroup.is_channel(),
        ChatType::Private(_) | ChatType::Secret(_) => message.is_outgoing(),
    };

    let text_content = match message.content().0 {
        MessageContent::MessageText(data) => dim_and_escape(&data.text.text),
        MessageContent::MessageBasicGroupChatCreate(_) => {
            show_sender = false;
            gettext!("{} created the group", sender_name(message.sender(), true))
        }
        MessageContent::MessageChatAddMembers(data) => {
            show_sender = false;

            if message.sender().as_user().map(User::id).as_ref() == data.member_user_ids.get(0) {
                if message.is_outgoing() {
                    gettext("You joined the group")
                } else {
                    gettext!("{} joined the group", sender_name(message.sender(), true))
                }
            } else {
                let session = message.chat().session();
                let user_list = session.user_list();

                let members = data
                    .member_user_ids
                    .into_iter()
                    .map(|user_id| user_list.get(user_id))
                    .map(|user| stringify_user(&user, true))
                    .collect::<Vec<_>>();

                let (last_member, first_members) = members.split_last().unwrap();

                gettext!(
                    "{} added {}",
                    sender_name(message.sender(), true),
                    if first_members.is_empty() {
                        Cow::Borrowed(last_member)
                    } else {
                        Cow::Owned(gettext!(
                            "{} and {}",
                            first_members.join(&gettext(", ")),
                            last_member
                        ))
                    }
                )
            }
        }
        MessageContent::MessageChatDeleteMember(data) => {
            show_sender = false;

            if message
                .sender()
                .as_user()
                .map(|user| user.id() == data.user_id)
                .unwrap_or_default()
            {
                if message.is_outgoing() {
                    gettext("You left the group")
                } else {
                    gettext!("{} left the group", sender_name(message.sender(), true))
                }
            } else {
                gettext!(
                    "{} removed {}",
                    sender_name(message.sender(), true),
                    stringify_user(
                        &message.chat().session().user_list().get(data.user_id),
                        true
                    )
                )
            }
        }
        MessageContent::MessageSticker(data) => {
            format!("{} {}", data.sticker.emoji, gettext("Sticker"))
        }
        MessageContent::MessagePhoto(data) => stringify_message_photo(&data.caption.text),
        MessageContent::MessageAudio(data) => {
            stringify_message_audio(&data.audio.performer, &data.audio.title, &data.caption.text)
        }
        MessageContent::MessageAnimation(data) => stringify_message_animation(&data.caption.text),
        MessageContent::MessageVideo(data) => stringify_message_video(&data.caption.text),
        MessageContent::MessageDocument(data) => {
            stringify_message_document(&data.document.file_name, &data.caption.text)
        }
        MessageContent::MessageVoiceNote(data) => stringify_message_voice_note(&data.caption.text),
        MessageContent::MessageCall(data) => {
            match data.discard_reason {
                CallDiscardReason::Declined => {
                    if message.is_outgoing() {
                        // Telegram Desktop/Android labels declined outgoing calls just as
                        // "Outgoing call" and puts a red arrow in the message bubble. We should be
                        // more accurate here.
                        if data.is_video {
                            gettext("Declined outgoing video call")
                        } else {
                            gettext("Declined outgoing call")
                        }
                    // Telegram Android labels declined incoming calls as "Incoming call". Telegram
                    // Desktop labels it as "Declined call" and is a bit inconsistent with outgoing
                    // calls ^.
                    } else if data.is_video {
                        gettext("Declined incoming video call")
                    } else {
                        gettext("Declined incoming call")
                    }
                }
                CallDiscardReason::Disconnected
                | CallDiscardReason::HungUp
                | CallDiscardReason::Empty => {
                    stringify_made_message_call(message.is_outgoing(), data)
                }
                CallDiscardReason::Missed => {
                    if message.is_outgoing() {
                        gettext("Cancelled call")
                    } else {
                        gettext("Missed call")
                    }
                }
            }
        }
        MessageContent::MessageChatDeletePhoto => {
            show_sender = false;

            match message.chat().type_() {
                ChatType::Supergroup(supergroup) if supergroup.is_channel() => {
                    gettext("Channel photo removed")
                }
                _ => {
                    if message.is_outgoing() {
                        gettext("You removed the group photo")
                    } else {
                        gettext!(
                            "{} removed the group photo",
                            sender_name(message.sender(), true)
                        )
                    }
                }
            }
        }
        MessageContent::MessageContactRegistered => {
            gettext!("{} joined Telegram", sender_name(message.sender(), true))
        }
        _ => gettext("Unsupported message"),
    };

    if show_sender {
        let sender_name = if message.is_outgoing() {
            gettext("You")
        } else {
            escape(&sender_name(message.sender(), false))
        };

        format!("{}: {}", sender_name, text_content)
    } else {
        text_content
    }
}

/// This method returns the text for all calls that have actually been made.
/// This means that the called party has accepted the call.
fn stringify_made_message_call(is_outgoing: bool, data: MessageCall) -> String {
    if is_outgoing {
        if data.duration > 0 {
            if data.is_video {
                gettext!(
                    "Outgoing video call ({})",
                    human_friendly_duration(data.duration)
                )
            } else {
                gettext!("Outgoing call ({})", human_friendly_duration(data.duration))
            }
        } else if data.is_video {
            gettext("Outgoing video call")
        } else {
            gettext("Outgoing call")
        }
    } else if data.duration > 0 {
        if data.is_video {
            gettext!(
                "Incoming video call ({})",
                human_friendly_duration(data.duration)
            )
        } else {
            gettext!("Incoming call ({})", human_friendly_duration(data.duration))
        }
    } else if data.is_video {
        gettext("Incoming video call")
    } else {
        gettext("Incoming call")
    }
}

fn stringify_draft_message(message: &DraftMessage) -> String {
    match &message.input_message_text {
        InputMessageContent::InputMessageAnimation(data) => {
            stringify_message_animation(data.caption.as_ref().map_or("", |c| &c.text))
        }
        InputMessageContent::InputMessageAudio(data) => stringify_message_audio(
            &data.performer,
            &data.title,
            data.caption.as_ref().map_or("", |c| &c.text),
        ),
        InputMessageContent::InputMessageDocument(data) => stringify_message_document(
            &gettext("Document"),
            data.caption.as_ref().map_or("", |c| &c.text),
        ),
        InputMessageContent::InputMessagePhoto(data) => {
            stringify_message_photo(data.caption.as_ref().map_or("", |c| &c.text))
        }
        InputMessageContent::InputMessageSticker(_) => gettext("Sticker"),
        InputMessageContent::InputMessageText(data) => dim_and_escape(&data.text.text),
        InputMessageContent::InputMessageVideo(data) => {
            stringify_message_video(data.caption.as_ref().map_or("", |c| &c.text))
        }
        InputMessageContent::InputMessageVoiceNote(data) => {
            stringify_message_voice_note(data.caption.as_ref().map_or("", |c| &c.text))
        }
        _ => gettext("Unsupported message"),
    }
}

fn stringify_message_animation(caption_text: &str) -> String {
    format!(
        "{}{}",
        gettext("GIF"),
        if caption_text.is_empty() {
            String::new()
        } else {
            format!(", {}", dim_and_escape(caption_text))
        }
    )
}

fn stringify_message_audio(performer: &str, title: &str, caption_text: &str) -> String {
    format!(
        "{} - {}{}",
        escape(performer),
        escape(title),
        if caption_text.is_empty() {
            String::new()
        } else {
            format!(", {}", dim_and_escape(caption_text))
        }
    )
}

fn stringify_message_document(file_name: &str, caption_text: &str) -> String {
    format!(
        "{}{}",
        escape(file_name),
        if caption_text.is_empty() {
            String::new()
        } else {
            format!(", {}", dim_and_escape(caption_text))
        }
    )
}

fn stringify_message_photo(caption_text: &str) -> String {
    format!(
        "{}{}",
        gettext("Photo"),
        if caption_text.is_empty() {
            String::new()
        } else {
            format!(", {}", dim_and_escape(caption_text))
        }
    )
}

fn stringify_message_video(caption_text: &str) -> String {
    format!(
        "{}{}",
        gettext("Video"),
        if caption_text.is_empty() {
            String::new()
        } else {
            format!(", {}", dim_and_escape(caption_text))
        }
    )
}

fn stringify_message_voice_note(caption_text: &str) -> String {
    format!(
        "{}{}",
        gettext("Voice message"),
        if caption_text.is_empty() {
            String::new()
        } else {
            format!(", {}", dim_and_escape(caption_text))
        }
    )
}

fn sender_name(sender: &MessageSender, use_full_name: bool) -> String {
    match sender {
        MessageSender::User(user) => stringify_user(user, use_full_name),
        MessageSender::Chat(chat) => chat.title(),
    }
}

fn stringify_user(user: &User, use_full_name: bool) -> String {
    if use_full_name {
        format!("{} {}", user.first_name(), user.last_name())
            .trim()
            .into()
    } else {
        user.first_name()
    }
}
