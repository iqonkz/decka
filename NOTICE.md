# Notice

Decka is licensed under the MIT License (see LICENSE).

Logo © Madi Seit, part of this repository, MIT.

## Acknowledgements

Decka was designed after studying the projects below. Ideas only, no code copied: not one line of code from any of them is used in Decka.

| Project | License | Idea taken |
|---|---|---|
| [Macro Deck](https://github.com/Macro-Deck-App/Macro-Deck) | Apache-2.0 (code; name and logo not covered) | The PC runs actions and the device only shows buttons; variables that drive button looks; multi-state buttons; a browser client |
| [Desko](https://github.com/typewriter03/Desko) | Desko Noncommercial License 1.0 (non-commercial) | Collectors write named sections of a shared state; broadcast only when a section changes; a collector publishes 
ull` when its source is absent |
| [OpenDeck](https://github.com/nekename/OpenDeck) | GPL-3.0-or-later | Confirmation that Tauri + Rust suits this class of application; toggle actions with a separate look per state |
| [ODeck](https://github.com/willianrod/ODeck) | MIT | Pairing by a QR code shown on the PC; a minimal action set (launch, hotkeys, media, open URL, pages) |
| [Xenon](https://github.com/marcimastro98/Xenon) | Custom non-commercial (source-available) | Pairing a phone with a code on the home network without an account; stating privacy boundaries in the README |
| [PhoneBox](https://github.com/ersinkoc/phonebox) | MIT | Idea of AI usage limits and phone approvals inspired by PhoneBox (github.com/ersinkoc/phonebox, MIT); no code copied |
| [Bitfocus Companion](https://github.com/bitfocus/companion) | MIT (not verified on the repository page) | "Feedback": a button's look is computed from external state, not from the local press |
| [Nixi](https://heynixi.com) (heynixi) | not applicable (idea only) | Voice control of the PC with a small floating pill and a local model |
| [VoiceOrbs](https://voiceorbs.vercel.app/orbs/particles-orb) | not applicable (idea only) | 3D Fibonacci-sphere particle orb visualization across voice states |

Voice control ideas inspired by Nixi (heynixi); no code copied. llama.cpp and whisper.cpp (MIT), Qwen3.5 (Apache-2.0), Whisper models (MIT) are downloaded at the user's request.

## Main dependencies

| Dependency | License |
|---|---|
| Tauri | MIT or Apache-2.0 |
| React | MIT |
| Capacitor | MIT |
| Lucide | ISC |
| Simple Icons (brand logos) | CC0-1.0; the logos are trademarks of their owners |
| axum | MIT |
| enigo | MIT |

Other dependencies keep their own licenses; see `Cargo.lock` and `package-lock.json` for the full list.
