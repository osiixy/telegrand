use gettextrs::gettext;
use gtk::{
    gdk,
    glib::{self, clone},
    prelude::*,
    subclass::prelude::*,
};
use tdgrand::{enums::AuthorizationState, functions, types};

use crate::{
    session::Session,
    session_manager::SessionManager,
    utils::{do_async, log_out, parse_formatted_text, send_tdlib_parameters},
};

mod imp {
    use super::*;
    use adw::subclass::prelude::BinImpl;
    use gtk::CompositeTemplate;
    use once_cell::sync::OnceCell;
    use std::cell::{Cell, RefCell};

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/com/github/melix99/telegrand/ui/login.ui")]
    pub struct Login {
        pub session_manager: OnceCell<SessionManager>,
        pub client_id: Cell<i32>,
        pub session: RefCell<Option<Session>>,
        pub tos_text: RefCell<String>,
        pub show_tos_popup: Cell<bool>,
        pub has_recovery_email_address: Cell<bool>,
        pub password_recovery_expired: Cell<bool>,
        #[template_child]
        pub outer_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub previous_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub previous_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub next_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub next_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub next_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub next_spinner: TemplateChild<gtk::Spinner>,
        #[template_child]
        pub content: TemplateChild<adw::Leaflet>,
        #[template_child]
        pub phone_number_entry: TemplateChild<gtk::Entry>,
        #[template_child]
        pub phone_number_use_qr_code_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub welcome_page_error_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub qr_code_image: TemplateChild<gtk::Image>,
        #[template_child]
        pub code_entry: TemplateChild<gtk::Entry>,
        #[template_child]
        pub code_error_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub registration_first_name_entry: TemplateChild<gtk::Entry>,
        #[template_child]
        pub registration_last_name_entry: TemplateChild<gtk::Entry>,
        #[template_child]
        pub registration_error_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub tos_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub password_entry: TemplateChild<gtk::PasswordEntry>,
        #[template_child]
        pub password_hint_action_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub password_hint_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub password_error_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub password_recovery_code_send_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub password_send_code_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub account_deletion_description_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub password_recovery_status_page: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub password_recovery_code_entry: TemplateChild<gtk::Entry>,
        #[template_child]
        pub password_recovery_error_label: TemplateChild<gtk::Label>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Login {
        const NAME: &'static str = "Login";
        type Type = super::Login;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            klass.install_action("login.previous", None, move |widget, _, _| {
                widget.previous()
            });
            klass.install_action("login.next", None, move |widget, _, _| widget.next());
            klass.install_action("login.use-qr-code", None, move |widget, _, _| {
                widget.request_qr_code();
            });
            klass.install_action(
                "login.go-to-forgot-password-page",
                None,
                move |widget, _, _| {
                    widget.navigate_to_page::<gtk::Editable, _, gtk::Widget>(
                        "password-forgot-page",
                        [],
                        None,
                        None,
                    );
                },
            );
            klass.install_action("login.recover-password", None, move |widget, _, _| {
                widget.recover_password();
            });
            klass.install_action(
                "login.show-no-email-access-dialog",
                None,
                move |widget, _, _| {
                    widget.show_no_email_access_dialog();
                },
            );
            klass.install_action(
                "login.show-delete-account-dialog",
                None,
                move |widget, _, _| {
                    widget.show_delete_account_dialog();
                },
            );
            klass.install_action("login.show-tos-dialog", None, move |widget, _, _| {
                widget.show_tos_dialog(false)
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Login {
        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            // On each page change, decide which button to hide/show and which actions to
            // (de)activate.
            self.content
                .connect_visible_child_name_notify(clone!(@weak obj => move |_| {
                    obj.update_actions_for_visible_page()
                }));

            self.tos_label.connect_activate_link(|label, _| {
                label
                    .activate_action("login.show-tos-dialog", None)
                    .unwrap();
                gtk::Inhibit(true)
            });

            // Disable all actions by default.
            obj.disable_actions();
        }
    }

    impl WidgetImpl for Login {}
    impl BinImpl for Login {}
}

glib::wrapper! {
    pub struct Login(ObjectSubclass<imp::Login>)
        @extends gtk::Widget, adw::Bin;
}

impl Default for Login {
    fn default() -> Self {
        Self::new()
    }
}

impl Login {
    pub fn new() -> Self {
        glib::Object::new(&[]).expect("Failed to create Login")
    }

    pub fn set_session_manager(&self, session_manager: SessionManager) {
        self.imp().session_manager.set(session_manager).unwrap();
    }

    pub fn login_client(&self, client_id: i32, session: Session) {
        let imp = self.imp();
        imp.client_id.set(client_id);

        imp.session.replace(Some(session));

        imp.phone_number_entry.set_text("");
        imp.registration_first_name_entry.set_text("");
        imp.registration_last_name_entry.set_text("");
        imp.code_entry.set_text("");
        imp.password_entry.set_text("");
    }

    pub fn set_authorization_state(&self, state: AuthorizationState) {
        let imp = self.imp();

        match state {
            AuthorizationState::WaitTdlibParameters => {
                let client_id = imp.client_id.get();
                let database_info = imp
                    .session
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .database_info()
                    .0
                    .clone();
                do_async(
                    glib::PRIORITY_DEFAULT_IDLE,
                    async move { send_tdlib_parameters(client_id, &database_info).await },
                    clone!(@weak self as obj => move |result| async move {
                        if let Err(err) = result {
                            show_error_label(
                                &obj.imp().welcome_page_error_label,
                                &err.message
                            );
                        }
                    }),
                );
            }
            AuthorizationState::WaitEncryptionKey(_) => {
                self.send_encryption_key();
            }
            AuthorizationState::WaitPhoneNumber => {
                // The page 'phone-number-page' is the first page and thus the visible page by
                // default. This means that no transition will happen when we receive
                // 'WaitPhoneNumber'. In this case, we have to update the actions manually.
                if imp.content.visible_child_name().unwrap() == "phone-number-page" {
                    self.update_actions_for_visible_page();
                }

                // Hide the spinner before entering 'phone-number-page'.
                imp.phone_number_use_qr_code_stack
                    .set_visible_child_name("image");

                self.navigate_to_page(
                    "phone-number-page",
                    [&*imp.phone_number_entry],
                    Some(&imp.welcome_page_error_label),
                    Some(&*imp.phone_number_entry),
                );
            }
            AuthorizationState::WaitCode(_) => {
                self.navigate_to_page(
                    "code-page",
                    [&*imp.code_entry],
                    Some(&imp.code_error_label),
                    Some(&*imp.code_entry),
                );
            }
            AuthorizationState::WaitOtherDeviceConfirmation(data) => {
                let size = imp.qr_code_image.pixel_size() as usize;
                let bytes_per_pixel = 3;

                let data_luma = qrcode_generator::to_image_from_str(
                    data.link,
                    qrcode_generator::QrCodeEcc::Low,
                    size,
                )
                .unwrap();

                let bytes = glib::Bytes::from_owned(
                    // gdk::Texture only knows 3 byte color spaces, thus convert Luma.
                    data_luma
                        .into_iter()
                        .flat_map(|p| (0..bytes_per_pixel).map(move |_| p))
                        .collect::<Vec<_>>(),
                );

                imp.qr_code_image
                    .set_paintable(Some(&gdk::MemoryTexture::new(
                        size as i32,
                        size as i32,
                        gdk::MemoryFormat::R8g8b8,
                        &bytes,
                        size * bytes_per_pixel,
                    )));

                self.navigate_to_page::<gtk::Editable, _, gtk::Widget>(
                    "qr-code-page",
                    [],
                    None,
                    None,
                );
            }
            AuthorizationState::WaitRegistration(data) => {
                imp.show_tos_popup.set(data.terms_of_service.show_popup);
                imp.tos_text
                    .replace(parse_formatted_text(data.terms_of_service.text));

                self.navigate_to_page(
                    "registration-page",
                    [
                        &*imp.registration_first_name_entry,
                        &*imp.registration_last_name_entry,
                    ],
                    Some(&imp.registration_error_label),
                    Some(&*imp.registration_first_name_entry),
                );
            }
            AuthorizationState::WaitPassword(data) => {
                // If we do RequestAuthenticationPasswordRecovery we will land in this arm again.
                // To avoid transition back, clearing the entries and to save cpu time, we check
                // whether we are in the password-forgot-page.
                if imp.content.visible_child_name().unwrap() == "password-forgot-page" {
                    return;
                }

                // When we enter the password page, the password to be entered should be masked by
                // default, so the peek icon is turned off and on again.
                imp.password_entry.set_show_peek_icon(false);
                imp.password_entry.set_show_peek_icon(true);

                imp.password_hint_action_row
                    .set_visible(!data.password_hint.is_empty());
                imp.password_hint_label.set_text(&data.password_hint);

                let account_deletion_preface = if data.has_recovery_email_address {
                    imp.password_recovery_status_page
                        .set_description(Some(&gettext!(
                            "The code was sent to {}.",
                            data.recovery_email_address_pattern
                        )));
                    gettext(
                            "One way to continue using your account is to delete your account and then recreate it"
                        )
                } else {
                    imp.password_recovery_status_page.set_description(None);
                    gettext(
                        "Since you have not provided a recovery email address, the only way to continue using your account is to delete your account and then recreate it"
                    )
                };

                imp.account_deletion_description_label.set_label(&format!(
                    "{}. {}",
                    account_deletion_preface,
                    gettext(
                        "Please note, you will lose all your chats and messages, along with any media and files you shared!"
                    )
                ));
                imp.password_recovery_code_send_box
                    .set_visible(data.has_recovery_email_address);
                imp.has_recovery_email_address
                    .set(data.has_recovery_email_address);

                // When we first enter WaitPassword, we assume that the mail with the recovery
                // code hasn't been sent, yet.
                imp.password_recovery_expired.set(true);

                self.navigate_to_page(
                    "password-page",
                    [&*imp.password_entry],
                    Some(&imp.password_error_label),
                    Some(&*imp.password_entry),
                );
            }
            AuthorizationState::Ready => {
                self.disable_actions();

                // Clear the qr code image save some potential memory.
                imp.qr_code_image.set_paintable(gdk::Paintable::NONE);

                imp.session_manager.get().unwrap().add_logged_in_session(
                    imp.client_id.get(),
                    imp.session.take().unwrap(),
                    true,
                );

                // Make everything invisible.
                imp.outer_box.set_visible(false);
            }
            _ => {}
        }
    }

    fn navigate_to_page<'a, E, I, W>(
        &self,
        page_name: &str,
        editables_to_clear: I,
        error_label_to_clear: Option<&gtk::Label>,
        widget_to_focus: Option<&W>,
    ) where
        E: IsA<gtk::Editable>,
        I: IntoIterator<Item = &'a E>,
        W: IsA<gtk::Widget>,
    {
        let imp = self.imp();

        // Before transition to the page, be sure to reset the error label because it still might
        // contain an error message from the time when it was previously visited.
        if let Some(error_label_to_clear) = error_label_to_clear {
            error_label_to_clear.set_label("");
        }
        // Also clear all editables on that page.
        editables_to_clear
            .into_iter()
            .for_each(|editable| editable.set_text(""));

        imp.content.set_visible_child_name(page_name);

        // Make sure everything is visible.
        imp.outer_box.set_visible(true);

        self.unfreeze();
        if let Some(widget_to_focus) = widget_to_focus {
            widget_to_focus.grab_focus();
        }
    }

    fn update_actions_for_visible_page(&self) {
        let imp = self.imp();

        let visible_page = imp.content.visible_child_name().unwrap();

        let is_previous_valid = imp
            .session_manager
            .get()
            .map(|session_manager| session_manager.sessions().n_items() > 0)
            .unwrap_or_default()
            || visible_page.as_str() != "phone-number-page";

        let is_next_valid = visible_page.as_str() != "password-forgot-page"
            && visible_page.as_str() != "qr-code-page";

        imp.previous_button.set_visible(is_previous_valid);
        imp.next_button.set_visible(is_next_valid);

        self.action_set_enabled("login.previous", is_previous_valid);
        self.action_set_enabled("login.next", is_next_valid);
        self.action_set_enabled("login.use-qr-code", visible_page == "phone-number-page");
        self.action_set_enabled(
            "login.go-to-forgot-password-page",
            visible_page == "password-page",
        );
        self.action_set_enabled(
            "login.recover-password",
            visible_page == "password-forgot-page" && imp.has_recovery_email_address.get(),
        );
        self.action_set_enabled(
            "login.show-no-email-access-dialog",
            visible_page == "password-recovery-page",
        );
        self.action_set_enabled(
            "login.show-delete-account-dialog",
            visible_page == "password-forgot-page",
        );
        self.action_set_enabled("login.show-tos-dialog", visible_page == "registration-page");
    }

    fn previous(&self) {
        let imp = self.imp();

        match imp.content.visible_child_name().unwrap().as_str() {
            "phone-number-page" => {
                self.freeze_with_previous_spinner();

                // Logout the client when login is aborted.
                log_out(imp.client_id.get());
                imp.session_manager.get().unwrap().switch_to_sessions(None);
            }
            "qr-code-page" => self.leave_qr_code_page(),
            "password-forgot-page" => self.navigate_to_page::<gtk::Editable, _, _>(
                "password-page",
                [],
                None,
                Some(&*imp.password_entry),
            ),
            "password-recovery-page" => self.navigate_to_page::<gtk::Editable, _, gtk::Widget>(
                "password-forgot-page",
                [],
                None,
                None,
            ),
            _ => self.navigate_to_page::<gtk::Editable, _, _>(
                "phone-number-page",
                [],
                None,
                Some(&*imp.phone_number_entry),
            ),
        }
    }

    fn next(&self) {
        self.freeze_with_next_spinner();

        let imp = self.imp();
        let visible_page = imp.content.visible_child_name().unwrap();

        match visible_page.as_str() {
            "phone-number-page" => self.send_phone_number(),
            "code-page" => self.send_code(),
            "registration-page" => {
                if imp.show_tos_popup.get() {
                    // Force the ToS dialog for the user before he can proceed
                    self.show_tos_dialog(true);
                } else {
                    // Just proceed if the user either doesn't need to accept the ToS
                    self.send_registration()
                }
            }
            "password-page" => self.send_password(),
            "password-recovery-page" => self.send_password_recovery_code(),
            other => unreachable!("no page named '{}'", other),
        }
    }

    fn request_qr_code(&self) {
        self.freeze();

        let imp = self.imp();
        imp.phone_number_use_qr_code_stack
            .set_visible_child_name("spinner");

        let other_user_ids = imp
            .session_manager
            .get()
            .unwrap()
            .logged_in_users()
            .into_iter()
            .map(|user| user.id())
            .collect();
        let client_id = imp.client_id.get();
        do_async(
            glib::PRIORITY_DEFAULT_IDLE,
            functions::request_qr_code_authentication(other_user_ids, client_id),
            clone!(@weak self as obj => move |result| async move {
                let imp = obj.imp();
                obj.handle_user_result(
                    result,
                    &imp.welcome_page_error_label,
                    &*imp.phone_number_entry
                );
            }),
        );
    }

    fn leave_qr_code_page(&self) {
        // We actually need to logout to stop tdlib sending us new links.
        // https://github.com/tdlib/td/issues/1645
        let imp = self.imp();
        let use_test_dc = imp
            .session
            .borrow()
            .as_ref()
            .unwrap()
            .database_info()
            .0
            .use_test_dc;

        log_out(imp.client_id.get());
        imp.session_manager
            .get()
            .unwrap()
            .add_new_session(use_test_dc);
    }

    fn show_tos_dialog(&self, user_needs_to_accept: bool) {
        let builder = gtk::MessageDialog::builder()
            .use_markup(true)
            .secondary_text(&*self.imp().tos_text.borrow())
            .modal(true)
            .transient_for(self.root().unwrap().downcast_ref::<gtk::Window>().unwrap());

        let dialog = if user_needs_to_accept {
            builder
                .buttons(gtk::ButtonsType::YesNo)
                .text(&gettext("Do You Accept the Terms of Service?"))
        } else {
            builder
                .buttons(gtk::ButtonsType::Ok)
                .text(&gettext("Terms of Service"))
        }
        .build();

        dialog.run_async(clone!(@weak self as obj => move |dialog, response| {
            if matches!(response, gtk::ResponseType::No) {
                // If the user declines the ToS, don't proceed and just stay in
                // the view but unfreeze it again.
                obj.unfreeze();
            } else if matches!(response, gtk::ResponseType::Yes) {
                // User has accepted the ToS, so we can proceed in the login
                // flow.
                obj.send_registration();
            }
            dialog.close();
        }));
    }

    fn disable_actions(&self) {
        self.action_set_enabled("login.previous", false);
        self.action_set_enabled("login.next", false);
        self.action_set_enabled("login.use-qr-code", false);
        self.action_set_enabled("login.go-to-forgot-password-page", false);
        self.action_set_enabled("login.recover-password", false);
        self.action_set_enabled("login.show-no-email-access-dialog", false);
        self.action_set_enabled("login.show-delete-account-dialog", false);
        self.action_set_enabled("login.show-tos-dialog", false);
    }

    fn freeze(&self) {
        self.disable_actions();
        self.imp().content.set_sensitive(false);
    }

    fn freeze_with_previous_spinner(&self) {
        self.freeze();

        self.imp().previous_stack.set_visible_child_name("spinner");
    }

    fn freeze_with_next_spinner(&self) {
        self.freeze();

        let imp = self.imp();
        imp.next_stack.set_visible_child(&imp.next_spinner.get());
    }

    fn unfreeze(&self) {
        let imp = self.imp();
        imp.previous_stack.set_visible_child_name("text");
        imp.next_stack.set_visible_child(&imp.next_label.get());
        imp.content.set_sensitive(true);
    }

    fn send_encryption_key(&self) {
        let client_id = self.imp().client_id.get();
        let encryption_key = "".to_string();
        do_async(
            glib::PRIORITY_DEFAULT_IDLE,
            functions::check_database_encryption_key(encryption_key, client_id),
            clone!(@weak self as obj => move |result| async move {
                if let Err(err) = result {
                    show_error_label(
                        &obj.imp().welcome_page_error_label,
                        &err.message
                    )
                }
            }),
        );
    }

    fn send_phone_number(&self) {
        let imp = self.imp();

        reset_error_label(&imp.welcome_page_error_label);

        let client_id = imp.client_id.get();
        let phone_number = imp.phone_number_entry.text();

        // Check if we are already have an account logged in with that phone_number.
        let phone_number_digits = phone_number
            .chars()
            .filter(|c| c.is_digit(10))
            .collect::<String>();

        let session_manager = imp.session_manager.get().unwrap();

        match session_manager.session_index_for(
            imp.session
                .borrow()
                .as_ref()
                .unwrap()
                .database_info()
                .0
                .use_test_dc,
            &phone_number_digits,
        ) {
            Some(pos) => {
                // We just figured out that we already have an open session for that account.
                // Therefore we logout the client, with which we wanted to log in and delete its
                // just created database directory.
                log_out(imp.client_id.get());
                imp.session_manager
                    .get()
                    .unwrap()
                    .switch_to_sessions(Some(pos));
            }
            None => {
                do_async(
                    glib::PRIORITY_DEFAULT_IDLE,
                    functions::set_authentication_phone_number(
                        phone_number.into(),
                        None,
                        client_id,
                    ),
                    clone!(@weak self as obj => move |result| async move {
                        let imp = obj.imp();
                        obj.handle_user_result(
                            result,
                            &imp.welcome_page_error_label,
                            &*imp.phone_number_entry
                        );
                    }),
                );
            }
        }
    }

    fn send_code(&self) {
        let imp = self.imp();

        reset_error_label(&imp.code_error_label);

        let client_id = imp.client_id.get();
        let code = imp.code_entry.text().to_string();
        do_async(
            glib::PRIORITY_DEFAULT_IDLE,
            functions::check_authentication_code(code, client_id),
            clone!(@weak self as obj => move |result| async move {
                let imp = obj.imp();
                obj.handle_user_result(result, &imp.code_error_label, &*imp.code_entry);
            }),
        );
    }

    fn send_registration(&self) {
        let imp = self.imp();

        reset_error_label(&imp.registration_error_label);

        let client_id = imp.client_id.get();
        let first_name = imp.registration_first_name_entry.text().to_string();
        let last_name = imp.registration_last_name_entry.text().to_string();
        do_async(
            glib::PRIORITY_DEFAULT_IDLE,
            functions::register_user(first_name, last_name, client_id),
            clone!(@weak self as obj => move |result| async move {
                let imp = obj.imp();
                obj.handle_user_result(
                    result,
                    &imp.registration_error_label,
                    &*imp.registration_first_name_entry
                );
            }),
        );
    }

    fn send_password(&self) {
        let imp = self.imp();

        reset_error_label(&imp.password_error_label);

        let client_id = imp.client_id.get();
        let password = imp.password_entry.text().to_string();
        do_async(
            glib::PRIORITY_DEFAULT_IDLE,
            functions::check_authentication_password(password, client_id),
            clone!(@weak self as obj => move |result| async move {
                let imp = obj.imp();
                obj.handle_user_result(
                    result,
                    &imp.password_error_label,
                    &*imp.password_entry
                );
            }),
        );
    }

    fn recover_password(&self) {
        let imp = self.imp();

        if imp.password_recovery_expired.get() {
            // We need to tell tdlib to send us the recovery code via mail (again).
            self.freeze();
            imp.password_send_code_stack
                .set_visible_child_name("spinner");

            let client_id = imp.client_id.get();
            do_async(
                glib::PRIORITY_DEFAULT_IDLE,
                functions::request_authentication_password_recovery(client_id),
                clone!(@weak self as obj => move |result| async move {
                    let imp = obj.imp();

                    // Remove the spinner from the button.
                    imp
                        .password_send_code_stack
                        .set_visible_child_name("image");

                    if result.is_ok() {
                        // Save that we do not need to resend the mail when we enter the recovery
                        // page the next time.
                        imp.password_recovery_expired.set(false);
                        obj.navigate_to_page(
                            "password-recovery-page",
                            [&*imp.password_recovery_code_entry],
                            Some(&imp.password_recovery_error_label),
                            Some(&*imp.password_recovery_code_entry),
                        );
                    } else {
                        obj.update_actions_for_visible_page();
                        // TODO: We also need to handle potiential errors here and inform the user.
                    }

                    obj.unfreeze();
                }),
            );
        } else {
            // The code has been send already via mail.
            self.navigate_to_page(
                "password-recovery-page",
                [&*imp.password_recovery_code_entry],
                Some(&imp.password_recovery_error_label),
                Some(&*imp.password_recovery_code_entry),
            );
        }
    }

    fn show_delete_account_dialog(&self) {
        let dialog = gtk::MessageDialog::builder()
            .text(&gettext("Warning"))
            .secondary_text(&gettext(
                "You will lose all your chats and messages, along with any media and files you shared!\n\nDo you want to delete your account?",
            ))
            .buttons(gtk::ButtonsType::Cancel)
            .modal(true)
            .transient_for(self.root().unwrap().downcast_ref::<gtk::Window>().unwrap())
            .build();

        dialog.add_action_widget(
            &gtk::Button::builder()
                .use_underline(true)
                .label("_Delete Account")
                .css_classes(vec!["destructive-action".to_string()])
                .build(),
            gtk::ResponseType::Accept,
        );

        dialog.run_async(clone!(@weak self as obj => move |dialog, response_id| {
            dialog.close();

            if matches!(response_id, gtk::ResponseType::Accept) {
                obj.freeze();
                let client_id = obj.imp().client_id.get();
                do_async(
                    glib::PRIORITY_DEFAULT_IDLE,
                    functions::delete_account(String::from("cloud password lost and not recoverable"), client_id),
                    clone!(@weak obj => move |result| async move {
                        // Just unfreeze in case of an error, else stay frozen until we are
                        // redirected to the welcome page.
                        if result.is_err() {
                            obj.update_actions_for_visible_page();
                            obj.unfreeze();
                            // TODO: We also need to handle potiential errors here and inform the
                            // user.
                        }
                    }),
                );
            } else {
                obj.imp()
                    .password_entry
                    .grab_focus();
            }
        }));
    }

    fn send_password_recovery_code(&self) {
        let imp = self.imp();
        let client_id = imp.client_id.get();
        let recovery_code = imp.password_recovery_code_entry.text().to_string();
        do_async(
            glib::PRIORITY_DEFAULT_IDLE,
            functions::recover_authentication_password(
                recovery_code,
                String::new(),
                String::new(),
                client_id,
            ),
            clone!(@weak self as obj => move |result| async move {
                let imp = obj.imp();

                if let Err(err) = result {
                    if err.message == "PASSWORD_RECOVERY_EXPIRED" {
                        // The same procedure is used as for the official client (as far as I
                        // understood from the code). Alternatively, we could send the user a new
                        // code, indicate that and stay on the recovery page.
                        imp.password_recovery_expired.set(true);
                        obj.navigate_to_page::<gtk::Editable, _, _>(
                            "password-page", [],
                            None,
                            Some(&*imp.password_entry)
                        );
                    } else {
                        obj.handle_user_error(
                            &err,
                            &imp.password_recovery_error_label,
                            &*imp.password_recovery_code_entry
                        );
                    }
                }
            }),
        );
    }

    fn show_no_email_access_dialog(&self) {
        let dialog = gtk::MessageDialog::builder()
            .text(&gettext("Sorry"))
            .secondary_text(&gettext(
                "If you can't restore access to the email, your remaining options are either to remember your password or to delete and recreate your account.",
            ))
            .buttons(gtk::ButtonsType::Close)
            .modal(true)
            .transient_for(self.root().unwrap().downcast_ref::<gtk::Window>().unwrap())
            .build();

        dialog.add_button(&gettext("_Go Back"), gtk::ResponseType::Other(0));

        dialog.run_async(clone!(@weak self as obj => move |dialog, response_id| {
            dialog.close();

            if let gtk::ResponseType::Other(_) = response_id {
                obj.navigate_to_page::<gtk::Editable, _, gtk::Widget>(
                    "password-forgot-page",
                    [],
                    None,
                    None,
                );
            } else {
                obj.imp()
                    .password_recovery_code_entry
                    .grab_focus();
            }
        }));
    }

    fn handle_user_result<T, W: IsA<gtk::Widget>>(
        &self,
        result: Result<T, types::Error>,
        error_label: &gtk::Label,
        widget_to_focus: &W,
    ) -> Option<T> {
        match result {
            Err(err) => {
                self.handle_user_error(&err, error_label, widget_to_focus);
                None
            }
            Ok(t) => Some(t),
        }
    }

    fn handle_user_error<W: IsA<gtk::Widget>>(
        &self,
        err: &types::Error,
        error_label: &gtk::Label,
        widget_to_focus: &W,
    ) {
        show_error_label(error_label, &err.message);
        // In case of an error we do not switch pages. So invalidate actions here.
        self.update_actions_for_visible_page();
        self.unfreeze();
        // Grab focus for entry again after error.
        widget_to_focus.grab_focus();
    }
}

fn show_error_label(error_label: &gtk::Label, message: &str) {
    error_label.set_text(message);
    error_label.set_visible(true);
}

fn reset_error_label(error_label: &gtk::Label) {
    error_label.set_text("");
    error_label.set_visible(false);
}
