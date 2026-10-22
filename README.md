<div align="center">
  <h1>AI Chat applet for COSMIC Desktop</h1>
  <br>
  <div>
    <img alt="chat" src="https://github.com/cosmic-utils/cosmic-ext-applet-ai/blob/main/screenshots/chat.png" width="280">
    <img alt="settings" src="https://github.com/cosmic-utils/cosmic-ext-applet-ai/blob/main/screenshots/settings.png" width="280">
  </div>
</div>

A COSMIC Desktop applet that talks to any OpenAI-compatible chat completion API —
OpenAI, OpenRouter, Groq, DeepSeek, LM Studio, or a self-hosted server.

# Configuration

Open the applet's **Settings** page and set:

| Setting      | Default                     | Description                                                        |
| ------------ | --------------------------- | ------------------------------------------------------------------ |
| API endpoint | `https://api.openai.com/v1` | Base URL of the OpenAI-compatible API. `/chat/completions` is appended |
| API key      | *(empty)*                   | Sent as `Authorization: Bearer <key>`                              |
| Model        | `gpt-4o-mini`               | Model identifier passed in the request payload                     |

Settings are stored in `~/.config/cosmic-ext-applet-ai/settings.ron`.

Some examples of endpoints:

- OpenAI — `https://api.openai.com/v1`
- OpenRouter — `https://openrouter.ai/api/v1`
- Groq — `https://api.groq.com/openai/v1`
- DeepSeek — `https://api.deepseek.com/v1`
- LM Studio (local) — `http://localhost:1234/v1`

# Installing this applet

Clone the repository, and use [just](https://github.com/casey/just)

If you don't have `just` installed, it is available in PopOS repository,
so you can install it with `apt`

```sh
sudo apt install just
```

or for Fedora

```sh
sudo dnf install just
```

Now you can clone repo and install applet.

```sh
git clone https://github.com/cosmic-utils/cosmic-ext-applet-ai.git
cd cosmic-ext-applet-ai
```

## Building

Run just:

```sh
just build-release
```

### Installing

```sh
sudo just install
```

Done

From now, you will be able to add applet to your desktop panel/dock
and chat with different models in real time :)

Developed by Anagh Barnwal  

## Known wgpu issue

There are currently some rendering issues with the `wgpu` libcosmic features
in some (older?) gpus. This doesn't affect the API, only the applet.
If you are affected by this, you can build and install it with this feature disabled:

```sh
just build-no-wgpu
sudo just install
```
