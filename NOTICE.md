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

## Wake word (downloaded when the user turns it on)

| Part | Name | License | SHA-256 |
|---|---|---|---|
| Model | [alphacep/vosk-model-small-streaming-ru](https://huggingface.co/alphacep/vosk-model-small-streaming-ru), revision `e18123ee` (Alpha Cephei), `encoder.int8.onnx` | Apache-2.0 | `e0db705e94ec35d803b1df4f40cda23d064e1142977c80ab288430b109777a9d` |
| | `decoder.onnx` | Apache-2.0 | `89b3088a9e20e1ef7f2e85ce1a3478afe6a9c4ac57369cabcc4beb8e95328ea0` |
| | `joiner.int8.onnx` | Apache-2.0 | `b55784b071ab7512eab4c7c44e4f5478284ef33c83562cc6a249b972515a31e5` |
| | `tokens.txt` | Apache-2.0 | `93bbbc0bae6b78c0bbb743d4aa9fded3bb5ff3aac5f0200e3a769a5a05e0fdf6` |
| Runtime | [sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx) 2.0.3, `win-x64-shared-MT-Release-no-tts-lib` (k2-fsa) | Apache-2.0; contains ONNX Runtime (MIT, Microsoft) | `06bfa330d96cba1963fc7f43df1049db785e0662724a15ff8eb8467d3086db16` |

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
