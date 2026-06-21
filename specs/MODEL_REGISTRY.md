# MODEL_REGISTRY.md — exact model sources (companion spec)

> Pins the **verified HuggingFace repos** for every model tier so the build never guesses. Repo IDs
> below were confirmed to exist. **Per-file GGUF/mmproj names are not hardcoded here on purpose** —
> resolve them at download time with the `hf` command in §4 so quant/mmproj names always match the
> repo's current contents. Wrong mmproj = llama-server crash (see §5), so follow §4 exactly.

All models download at runtime to the app models dir (never committed). Recommended quant for all
GGUF: **Q4_K_M** (2026 default balance); offer Q5_K_M/Q6_K as a higher-quality option in settings.

## 1. Vision lane (image-text-to-text; **requires mmproj** from the same repo)

| Tier | Repo ID | License | Notes |
|---|---|---|---|
| **Default** | `Qwen/Qwen3-VL-4B-Instruct-GGUF` | Apache-2.0 | Official Qwen. Light on-demand tagging. |
| **Quality** | `Qwen/Qwen3-VL-8B-Instruct-GGUF` | Apache-2.0 | Official Qwen. Higher-fidelity descriptions. |
| **Beta** | `jc-builds/Qwen3.5-9B-VLM-Q4_K_M-GGUF` | Apache-2.0 | Newest gen; `llama.cpp`-native repo; experimental. |

> Use the **Instruct** variants (not `*-Thinking`) for vision tagging. Each repo ships its matching
> `mmproj-*.gguf` — always take the projector from the **same repo** as the model weights.

## 2. Answer lane (text-generation, *thinking*; **no mmproj**)

| Tier | Repo ID | Ctx | License | Notes |
|---|---|---|---|---|
| **Default** | `unsloth/Ministral-3-3B-Reasoning-2512-GGUF` | 256K | Apache-2.0 | Lightest (3B), vanilla `mistral3` arch — rock-solid in llama.cpp. |
| **Quality** | `unsloth/Qwen3-4B-Thinking-2507-GGUF` | 256K | Apache-2.0 | Top small reasoner; same family as vision. |
| **Beta** | `nvidia/NVIDIA-Nemotron-3-Nano-4B-GGUF` | ~49K | ⚠️ **NVIDIA OML** (`license: other`) | Strong reasoner, **hybrid Mamba-Transformer** — verify llama.cpp/Vulkan behavior; Beta only, never the default. Confirm OML terms before redistribution. |

## 3. Embeddings (in-process **fastembed**, not the sidecar — no manual GGUF)

| Role | Model | Dim | Notes |
|---|---|---|---|
| Text | EmbeddingGemma-300M (fastembed variant `EmbeddingGemma300MQ`) | 768 | fastembed downloads it on first use. **Cannot batch** — embed one input at a time. |
| Image (optional) | nomic-embed-vision-v1.5 | 768 | fastembed; enables OCR-less visual recall. |

> Do **not** use Qwen3-VL-Embedding via llama.cpp — it ignores images (proven). Image vectors come
> from fastembed only.

## 4. Resolve exact filenames at download time (do this, don't hardcode)
```bash
# Vision tier (model + matching projector):
hf download Qwen/Qwen3-VL-4B-Instruct-GGUF --include "*Q4_K_M*.gguf" "mmproj*.gguf" \
  --local-dir <models>/vision/qwen3-vl-4b
# inspect what's there if unsure:
hf download Qwen/Qwen3-VL-4B-Instruct-GGUF --include "*.gguf" --dry-run   # or browse the repo files

# Answer tier (no mmproj):
hf download unsloth/Ministral-3-3B-Reasoning-2512-GGUF --include "*Q4_K_M*.gguf" \
  --local-dir <models>/answer/ministral-3-3b-reasoning
```
The supervisor's `ModelSpec` (`03 §6`) gets `gguf_path` (+ `mmproj_path` for vision) from the
resolved files. Launch flags: `-ngl <sidecar.ngl>`; vision adds `--mmproj <mmproj_path>`.

## 5. Hard invariants (do not violate)
- **mmproj must be same-family** as the active vision model (mismatch crashes llama-server).
- **Vision uses Instruct variants**, not Thinking.
- **EmbeddingGemma-Q cannot batch** — one input per embed call.
- **Nemotron is Beta-only** (hybrid arch + non-Apache license); Default/Quality stay vanilla-arch +
  Apache so the common path is always proven.
- Defaults on first run: **vision = Qwen3-VL-4B-Instruct**, **answer = Ministral-3-3B-Reasoning**.
