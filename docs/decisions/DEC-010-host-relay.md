# DEC-010: Permit a development-host OpenAI relay

- **Status:** Superseded by DEC-011 on 2026-09-19
- **Decision:** The original design permitted a development-host relay for production OpenAI calls.
- **Consequences:** The hackathon deployment no longer calls OpenAI and cannot use it as fallback. A local companion host may run `llama.cpp`; its placement and loopback boundary must be explicit in the setup guide.
- **Revisit when:** Historical record only; use DEC-011 for current requirements.
