use ashpd::desktop::file_chooser::{FileFilter, SelectedFiles};
use cosmic::{
    app::Core,
    applet::padded_control,
    iced::{
        self,
        alignment::Horizontal,
        id,
        platform_specific::shell::wayland::commands::popup::{destroy_popup, get_popup},
        stream::channel,
        theme::Palette,
        window::Id,
        Length, Subscription,
    },
    iced_widget::{
        scrollable::{snap_to, RelativeOffset},
        Scrollable,
    },
    theme,
    widget::{self, settings},
    Action, Application, Element, Task as Command,
};
use futures::{SinkExt as _, StreamExt as _};
use std::{path::PathBuf, pin::pin};

use crate::{
    chat::{
        load_conversation, read_conversation_files, Conversation, Image, ImageAttachment,
        MessageContent, Text,
    },
    fl,
    models::available_models,
    stream::{self, service},
    Settings,
};

const ID: &str = "dev.heppen.ai";

#[derive(Debug, Clone)]
pub enum Pages {
    Chat,
    Settings,
}

#[derive(Debug, Clone)]
pub enum StreamingRequest {
    Idle,
    Ask,
    AskWithContext,
}

#[derive(Debug, Clone)]
pub enum Message {
    ChatPage,
    SettingsPage,
    PopupClosed(Id),
    TogglePopup,
    EnterPrompt(String),
    SendPrompt(String),
    ChangeModel(usize),
    ClearChat,
    BotEvent(stream::Event),
    ToggleContext,
    StopBot,
    SaveConversation,
    SelectedConversation(usize),
    LoadConversation,
    RemoveConversation,
    OpenImages,
    ImagesResult(Vec<Image>),
    FindAvatar,
    AvatarResult(PathBuf),
    ApiEndpointFlag(bool),
    ApiEndpointInput(String),
    ApiEndpointSend(String),
    ApiKeyInput(String),
    ApiKeySend(String),
    OpenLink(iced::widget::markdown::Url),
    RefreshModels,
}

pub struct Window {
    core: Core,
    popup: Option<Id>,
    page: Pages,
    prompt: String,
    conversation: Conversation,
    bot_response: String,
    system_messages: Vec<String>,
    models: Vec<String>,
    selected_model: String,
    model_index: Option<usize>,
    last_id: usize,
    chat_id: id::Id,
    keep_context: bool,
    images: Vec<Image>,
    saved_conversations: Vec<String>,
    selected_saved_conv: Option<usize>,
    request: StreamingRequest,
    status_area_status: String,
    user_avatar: widget::image::Handle,
    api_endpoint: String,
    api_endpoint_edit: bool,
    api_key: String,
    settings: Settings,
}

impl Application for Window {
    type Executor = cosmic::SingleThreadExecutor;

    type Flags = ();

    type Message = Message;

    const APP_ID: &'static str = ID;

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, _flags: Self::Flags) -> (Self, Command<Action<Message>>) {
        let system_messages = Vec::new();
        let models: Vec<String> = available_models();
        let settings = Settings::load();
        let model_index = models
            .clone()
            .into_iter()
            .position(|model| model == settings.model);

        (
            Self {
                core,
                popup: None,
                page: Pages::Chat,
                prompt: String::new(),
                conversation: Conversation::new(),
                bot_response: String::new(),
                system_messages,
                models: models.clone(),
                selected_model: settings.model.clone(),
                model_index,
                last_id: 0,
                chat_id: id::Id::new("chat"),
                keep_context: settings.keep_context,
                images: Vec::new(),
                saved_conversations: Vec::new(),
                selected_saved_conv: Some(0),
                request: StreamingRequest::Idle,
                status_area_status: String::new(),
                user_avatar: settings.get_avatar_handle(),
                api_endpoint: settings.api_endpoint.clone(),
                api_endpoint_edit: false,
                api_key: settings.api_key.clone(),
                settings,
            },
            Command::none(),
        )
    }

    fn on_close_requested(&self, id: Id) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }

    fn subscription(&self) -> Subscription<Message> {
        let stream = channel(4, move |mut output| async move {
            let mut stream = pin!(service());

            while let Some(event) = stream.next().await {
                let _res = output.send(Message::BotEvent(event)).await;
            }
        });

        Subscription::run_with_id(self.last_id, stream)
    }

    fn update(&mut self, message: Message) -> Command<Action<Message>> {
        let mut commands: Vec<Command<Action<Message>>> = Vec::new();

        match message {
            Message::ChatPage => {
                self.page = Pages::Chat;
                commands.push(snap_to(self.chat_id.clone(), RelativeOffset::END));
            }
            Message::SettingsPage => {
                self.page = Pages::Settings;

                self.saved_conversations = read_conversation_files().unwrap_or_default();
            }
            Message::PopupClosed(id) => {
                if Some(id) == self.popup {
                    self.popup = None;
                }
            }
            Message::TogglePopup => {
                commands.push(if let Some(p) = self.popup.take() {
                    destroy_popup(p)
                } else {
                    let new_id = Id::unique();
                    self.popup.replace(new_id);
                    let Some(main_id) = self.core.main_window_id() else {
                        return Command::none();
                    };
                    let mut popup_settings =
                        self.core.applet.get_popup_settings(main_id, new_id, None, None, None);
                    popup_settings.positioner.size_limits =
                        iced::Limits::NONE.width(680.0).height(800.0);
                    return get_popup(popup_settings);
                });
            }
            Message::EnterPrompt(prompt) => self.prompt = prompt,
            Message::SendPrompt(prompt) => {
                self.conversation
                    .push(Text::User(MessageContent::Text(prompt)));
                self.last_id += 1;

                if !self.keep_context {
                    self.request = StreamingRequest::Ask
                } else {
                    self.request = StreamingRequest::AskWithContext
                }
            }
            Message::BotEvent(ev) => match ev {
                stream::Event::Ready(tx) => {
                    match self.request {
                        StreamingRequest::Ask => {
                            // Images are carried as (mime_type, base64) tuples.
                            let images: Vec<(String, String)> = self
                                .images
                                .iter()
                                .map(|img| (img.mime(), img.base64.clone()))
                                .collect();

                            _ = tx.blocking_send(stream::Request::Ask((
                                self.selected_model.clone(),
                                self.prompt.clone(),
                                images,
                            )))
                        }
                        StreamingRequest::AskWithContext => {
                            // Convert the conversation history to (role, content) pairs.
                            let history: Vec<(String, String)> = self
                                .conversation
                                .messages
                                .iter()
                                .map(|msg| {
                                    let role = match msg {
                                        Text::User(_) => "user",
                                        Text::Bot(_) => "assistant",
                                    };
                                    let content = match msg {
                                        Text::User(MessageContent::Text(t))
                                        | Text::Bot(MessageContent::Text(t)) => t.clone(),
                                        _ => String::new(),
                                    };
                                    (role.to_string(), content)
                                })
                                .filter(|(_, c)| !c.is_empty())
                                .collect();

                            // Images are carried as (mime_type, base64) tuples.
                            let images: Vec<(String, String)> = self
                                .images
                                .iter()
                                .map(|img| (img.mime(), img.base64.clone()))
                                .collect();

                            _ = tx.blocking_send(stream::Request::AskWithContext((
                                self.selected_model.clone(),
                                self.prompt.clone(),
                                images,
                                history,
                            )))
                        }
                        StreamingRequest::Idle => {}
                    }
                    self.prompt.clear();
                }
                stream::Event::Response(message) => {
                    self.bot_response.push_str(&message.response);
                    commands.push(snap_to(self.chat_id.clone(), RelativeOffset::END));
                }
                stream::Event::Error(msg) => {
                    self.status_area_status = msg;
                    commands.push(snap_to(self.chat_id.clone(), RelativeOffset::END));
                }
                stream::Event::Done => {
                    if !self.bot_response.is_empty() {
                        self.conversation
                            .push(Text::Bot(MessageContent::Text(self.bot_response.clone())));
                        self.bot_response.clear();
                    }
                    self.images.clear();
                    self.request = StreamingRequest::Idle;
                }
            },
            Message::ChangeModel(index) => {
                self.model_index = Some(index);
                self.selected_model.clone_from(&self.models[index]);
                self.settings.set_model(self.selected_model.clone());
                if let Err(err) = self.settings.save() {
                    self.status_area_status = format!("save settings failed: {err}");
                }
            }
            Message::ClearChat => {
                self.prompt.clear();
                self.system_messages.clear();
                self.conversation = Conversation::new();
            }
            Message::ToggleContext => {
                self.keep_context = !self.keep_context;
                self.settings.change_context(self.keep_context);
                if let Err(err) = self.settings.save() {
                    self.status_area_status = format!("save settings failed: {err}");
                }
            }
            Message::StopBot => {
                self.last_id += 1;
                // Cancel any in-flight stream so it doesn't restart on the new subscription.
                self.request = StreamingRequest::Idle;
                self.bot_response.clear();
                self.prompt.clear();
            },
            Message::SaveConversation => {
                if let Err(err) = self.conversation.save_to_file() {
                    self.status_area_status = format!("save failed: {err}");
                }
            }
            Message::SelectedConversation(index) => {
                self.selected_saved_conv = Some(index);
            }
            Message::LoadConversation => {
                if let Some(selected_saved_conv) = self.selected_saved_conv {
                    if self.saved_conversations.len() > selected_saved_conv {
                        match load_conversation(
                            self.saved_conversations[selected_saved_conv].clone(),
                        ) {
                            Ok(conversation) => self.conversation = conversation,
                            Err(err) => self.status_area_status = format!("{err}"),
                        }
                    }
                }
            }
            Message::RemoveConversation => {
                if let Some(selected_saved_conv) = self.selected_saved_conv {
                    if self.saved_conversations.len() > selected_saved_conv {
                        if let Err(err) = self
                            .conversation
                            .remove(self.saved_conversations[selected_saved_conv].clone())
                        {
                            self.status_area_status = format!("remove failed: {err}");
                        }
                    }
                }

                self.saved_conversations = read_conversation_files().unwrap_or_default();
            }
            Message::OpenImages => {
                commands.push(Command::perform(
                    async move {
                        let Ok(response) = SelectedFiles::open_file()
                            .title("Open multiple images")
                            .accept_label("Attach")
                            .modal(true)
                            .multiple(true)
                            .filter(FileFilter::new("JPEG Image").glob("*.jpg"))
                            .filter(FileFilter::new("PNG Image").glob("*.png"))
                            .send()
                            .await
                            .and_then(|s| s.response())
                        else {
                            return Vec::new();
                        };

                        response
                            .uris()
                            .iter()
                            .filter_map(|file| Image::new(file.path()).ok())
                            .collect::<Vec<Image>>()
                    },
                    |files| Message::ImagesResult(files).into(),
                ));
            }
            Message::ImagesResult(result) => {
                for image in result {
                    self.images.push(image.clone());
                    self.conversation.push(Text::User(MessageContent::Image(
                        ImageAttachment::Raster(image),
                    )));
                }
            }
            Message::FindAvatar => commands.push(Command::perform(
                async move {
                    let Ok(response) = SelectedFiles::open_file()
                        .title("Open image")
                        .accept_label("Attach")
                        .modal(true)
                        .multiple(false)
                        .filter(FileFilter::new("JPEG Image").glob("*.jpg"))
                        .filter(FileFilter::new("PNG Image").glob("*.png"))
                        .send()
                        .await
                        .and_then(|s| s.response())
                    else {
                        return PathBuf::new();
                    };

                    response
                        .uris()
                        .first()
                        .and_then(|u| u.to_file_path().ok())
                        .unwrap_or_default()
                },
                |path| Message::AvatarResult(path).into(),
            )),
            Message::AvatarResult(path) => {
                if path.as_os_str().is_empty() {
                    return Command::none(); // user cancelled picker; keep existing avatar
                }
                let handle = widget::image::Handle::from_path(&path);
                self.user_avatar = handle;
                self.settings.set_avatar(path);
                if let Err(err) = self.settings.save() {
                    self.status_area_status = format!("save avatar failed: {err}");
                }
            }
            Message::ApiEndpointFlag(flag) => self.api_endpoint_edit = flag,
            Message::ApiEndpointInput(input) => self.api_endpoint = input,
            Message::ApiEndpointSend(endpoint) => {
                self.settings.set_api_endpoint(endpoint.clone());
                if let Err(err) = self.settings.save() {
                    self.status_area_status = format!("save settings failed: {err}");
                }
                self.api_endpoint = endpoint;
                // Refresh available models when endpoint changes.
                self.models = available_models();
                self.model_index = self
                    .models
                    .clone()
                    .into_iter()
                    .position(|m| m == self.selected_model);
            }
            Message::ApiKeyInput(input) => self.api_key = input,
            Message::ApiKeySend(key) => {
                self.settings.set_api_key(key);
                if let Err(err) = self.settings.save() {
                    self.status_area_status = format!("save settings failed: {err}");
                }
                // Refresh available models from new endpoint/key.
                self.models = available_models();
                self.model_index = self
                    .models
                    .clone()
                    .into_iter()
                    .position(|m| m == self.selected_model);
            }
            Message::RefreshModels => {
                self.models = available_models();
                self.model_index = self
                    .models
                    .clone()
                    .into_iter()
                    .position(|m| m == self.selected_model);
            }
            Message::OpenLink(url) => {
                let _ = open::that_in_background(url.to_string());
            }
        };

        Command::batch(commands)
    }

    fn view(&self) -> Element<'_, Self::Message> {
        self.core
            .applet
            .icon_button("dev.heppen.ai-symbolic")
            .on_press(Message::TogglePopup)
            .into()
    }

    fn view_window(&self, _id: Id) -> Element<'_, Self::Message> {
        let menu_row = widget::row().push(padded_control(self.menu_bar()));

        let page_view = match self.page {
            Pages::Chat => self.chat_view(),
            Pages::Settings => self.settings_view(),
        };

        let content_list = widget::column()
            .push(padded_control(menu_row))
            .push(padded_control(page_view))
            .padding(10);

        self.core
            .applet
            .popup_container(content_list)
            .limits(iced::Limits::NONE.max_width(680.0).max_height(800.0))
            .into()
    }
}

impl Window {
    fn chat_view(&self) -> Element<'_, Message> {
        let mut chat = widget::column()
            .spacing(10)
            .padding(12.0)
            .width(Length::Fill);

        chat = chat.push(self.chat_messages(&self.conversation));

        if !self.bot_response.is_empty() {
            chat = chat.push(self.bot_bubble(self.bot_response.clone()));
        }

        for message in &self.system_messages {
            chat = chat.push(self.system_bubble(message.to_string()))
        }

        let prompt_input = widget::text_input(fl!("prompt-field"), &self.prompt)
            .on_input(Message::EnterPrompt)
            .on_submit(Message::SendPrompt)
            .width(Length::Fill);

        let open_images = widget::button::icon(widget::icon::from_name("mail-attachment-symbolic"))
            .on_press(Message::OpenImages);

        let clear_chat = widget::button::icon(widget::icon::from_name("edit-clear-symbolic"))
            .on_press(Message::ClearChat);

        let stop_bot =
            widget::button::icon(widget::icon::from_name("media-playback-stop-symbolic"))
                .on_press(Message::StopBot);

        let fields = widget::row()
            .push(prompt_input)
            .push(open_images)
            .push(clear_chat)
            .push(stop_bot)
            .spacing(10);

        widget::column()
            .push(padded_control(
                widget::Container::new(Scrollable::new(chat).id(self.chat_id.clone()))
                    .height(Length::Fill),
            ))
            .push(padded_control(fields))
            .height(Length::Fill)
            .into()
    }

    //noinspection ALL
    fn settings_view(&self) -> Element<'_, Message> {
        let conv_section = settings::section::section()
            .title(fl!("conversations"))
            .add(settings::item(
                fl!("user-avatar"),
                widget::button::standard(fl!("open")).on_press(Message::FindAvatar),
            ))
            .add(settings::item(
                fl!("keep-context"),
                widget::toggler(self.keep_context).on_toggle(|_| Message::ToggleContext),
            ))
            .add(settings::item(
                fl!("select-conversation"),
                widget::dropdown(
                    &self.saved_conversations,
                    self.selected_saved_conv,
                    Message::SelectedConversation,
                ),
            ))
            .add(settings::item(
                fl!("load-conversation"),
                widget::button::standard(fl!("load")).on_press(Message::LoadConversation),
            ))
            .add(settings::item(
                fl!("save-conversation"),
                widget::button::standard(fl!("save")).on_press(Message::SaveConversation),
            ))
            .add(settings::item(
                fl!("remove-conversation"),
                widget::button::standard(fl!("remove")).on_press(Message::RemoveConversation),
            ));

        let connection_section = settings::section()
            .title(fl!("manage-models"))
            .add(settings::item(
                fl!("api-endpoint"),
                widget::editable_input(
                    "https://api.openai.com/v1",
                    &self.api_endpoint,
                    self.api_endpoint_edit,
                    Message::ApiEndpointFlag,
                )
                .on_input(Message::ApiEndpointInput)
                .on_submit(Message::ApiEndpointSend),
            ))
            .add(settings::item(
                fl!("api-key"),
                widget::text_input("", &self.api_key)
                    .on_input(Message::ApiKeyInput)
                    .on_submit(Message::ApiKeySend),
            ))
            .add(settings::item(
                fl!("select-model"),
                widget::dropdown(
                    &self.models,
                    self.model_index,
                    Message::ChangeModel,
                )
                .width(Length::Fill),
            ))
            .add(settings::item(
                fl!("refresh-models"),
                widget::button::standard(fl!("refresh")).on_press(Message::RefreshModels),
            ));

        let spacer = widget::Space::with_height(Length::Fill);

        let status_area = widget::row()
            .push(widget::text::monotext("Status: "))
            .push(widget::text::monotext(self.status_area_status.clone()))
            .spacing(10);

        let mut content = widget::column()
            .push(conv_section)
            .push(connection_section)
            .push(spacer)
            .spacing(20);

        if !self.status_area_status.is_empty() {
            content = content.push(status_area);
        }

        widget::Container::new(padded_control(content))
            .height(Length::Fill)
            .into()
    }

    fn bot_bubble(&self, message: String) -> Element<'_, Message> {
        let content: Vec<iced::widget::markdown::Item> =
            widget::markdown::parse(&message).collect();
        let markdown = iced::widget::markdown(
            &content,
            iced::widget::markdown::Settings::default(),
            iced::widget::markdown::Style::from_palette(Palette::DARK),
        )
        .map(Message::OpenLink);

        let ai = widget::Container::new(markdown)
            .padding(12)
            .class(theme::Container::List);

        let avatar: &[u8] = include_bytes!("../data/icons/avatar.png");
        let handle = widget::image::Handle::from_bytes(avatar);
        let avatar_widget = widget::image(handle)
            .width(Length::Fixed(48.0))
            .height(Length::Fixed(48.0));

        let message_row = widget::row().push(avatar_widget).push(ai).spacing(12);
        let content = widget::column().push(message_row);

        widget::Container::new(content).into()
    }

    fn user_bubble(&self, message: &MessageContent) -> Element<'_, Message> {
        let mut column = widget::column();

        match message {
            MessageContent::Image(image) => match image {
                ImageAttachment::Svg(_) => {
                    column = column.push(widget::text("[SVG attachment]"));
                }
                ImageAttachment::Raster(raster) => {
                    let handle = widget::image::Handle::from_bytes(raster.data.clone());

                    column = column.push(widget::image(handle))
                }
            },
            MessageContent::Text(txt) => {
                if !txt.is_empty() {
                    let content: Vec<iced::widget::markdown::Item> =
                        iced::widget::markdown::parse(txt).collect();
                    let markdown = iced::widget::markdown(
                        &content,
                        iced::widget::markdown::Settings::default(),
                        iced::widget::markdown::Style::from_palette(Palette::DARK),
                    )
                    .map(Message::OpenLink);
                    column = column.push(markdown)
                }
            }
        };

        let margin = widget::row().width(50).height(50);

        let container = widget::container(column)
            .padding(12)
            .class(theme::Container::List);

        let avatar_widget = widget::image(self.user_avatar.clone())
            .width(Length::Fixed(48.0))
            .height(Length::Fixed(48.0));

        let message_row = widget::row()
            .push(margin)
            .push(container)
            .push(avatar_widget)
            .spacing(12);

        widget::Container::new(message_row)
            .width(Length::Fill)
            .align_x(Horizontal::Right)
            .into()
    }

    fn chat_messages(&self, conv: &Conversation) -> Element<'_, Message> {
        let mut content = widget::column().spacing(20);

        for c in &conv.messages {
            match c {
                Text::User(message) => content = content.push(self.user_bubble(message)),
                Text::Bot(text) => match text {
                    MessageContent::Text(txt) => {
                        if !txt.is_empty() {
                            content = content.push(self.bot_bubble(txt.clone()))
                        }
                    }
                    MessageContent::Image(_) => {
                        // Bot image replies: render as placeholder text in message stream.
                        content = content.push(widget::text("[Bot image response — not yet rendered]").size(14));
                    }
                },
            }
        }

        widget::Container::new(content).width(Length::Fill).into()
    }

    fn system_bubble(&self, message: String) -> Element<'_, Message> {
        let user = widget::Container::new(
            widget::Container::new(widget::text(message))
                .padding(12)
                .class(theme::Container::List),
        )
        .width(Length::Fill)
        .align_x(Horizontal::Right);

        let user_col = widget::column().push(user);

        let content = widget::column().push(user_col).spacing(10);

        widget::Container::new(content).width(Length::Fill).into()
    }

    fn menu_bar(&self) -> Element<'_, Message> {
        settings::section()
            .title("")
            .add(settings::item_row(vec![
                widget::button::standard(fl!("chat"))
                    .on_press(Message::ChatPage)
                    .into(),
                widget::button::standard(fl!("settings"))
                    .on_press(Message::SettingsPage)
                    .into(),
                widget::dropdown(&self.models, self.model_index, Message::ChangeModel)
                    .width(Length::Fill)
                    .into(),
            ]))
            .into()
    }
}
