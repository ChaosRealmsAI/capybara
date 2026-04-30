# External Reference Repository Manifest

These repositories are local-only reference clones for Capybara. The clones live
under `reference-repos/github/`, which is ignored by git. This manifest is
tracked so future agents know what was downloaded, from where, at which commit,
and why it matters.

Last refreshed: 2026-04-30.

## Highest-Signal Starting Points

Read these first for the current "Remotion + Lovart + Claude Design" direction:

- `heygen-com__hyperframes`: HTML-to-video built for agents; very close to Capybara timeline/video output.
- `remotion-dev__remotion`: React video programming model; useful but has a special license.
- `nexu-io__open-design`, `OpenCoworkAI__open-codesign`, `superdesigndev__superdesign`, `ZSeven-W__openpencil`: open Claude Design / AI design agent references.
- `dyad-sh__dyad`, `firecrawl__open-lovable`, `stackblitz__bolt.new`, `e2b-dev__fragments`: prompt-to-app and sandboxed preview references.
- `excalidraw__excalidraw`, `tldraw__tldraw`, `penpot__penpot`, `GraphiteEditor__Graphite`: canvas/editor state and interaction architecture.
- `cline__cline`, `sst__opencode`, `continuedev__continue`, `aider-ai__aider`, `OpenHands__OpenHands`: high-quality agent/tool orchestration references.

## Repositories

| Category | Local path | Source URL | Commit | Use in Capybara |
|---|---|---|---|---|
| AI codegen baseline | `github/AntonOsika__gpt-engineer` | https://github.com/AntonOsika/gpt-engineer.git | `a90fcd543eedcc0ff2c34561bc0785d2ba83c47e` | Lovable precursor; inspect prompt-to-code loop and project generation boundaries. |
| Cross-framework components | `github/BuilderIO__mitosis` | https://github.com/BuilderIO/mitosis.git | `e4255db77c3a202aef001963944d5e219c2d0c3f` | Study component IR and multi-target generation ideas for design output portability. |
| Image workflow graph | `github/Comfy-Org__ComfyUI` | https://github.com/Comfy-Org/ComfyUI.git | `a164c82913d3e04d92d0f6630fc4c850ec184ef3` | Node workflow, provider/tool graph, image generation orchestration reference. |
| AI workflow builder | `github/FlowiseAI__Flowise` | https://github.com/FlowiseAI/Flowise.git | `133aacf1d5475be747431939fa26a286ae4ec47e` | Visual AI workflow builder and node registry reference. |
| Web builder | `github/GrapesJS__grapesjs` | https://github.com/GrapesJS/grapesjs.git | `cf8257ad6f15de797f44e48bc4d199edb6a9db10` | Mature web builder plugin/model architecture reference. |
| Vector editor | `github/GraphiteEditor__Graphite` | https://github.com/GraphiteEditor/Graphite.git | `e686ee9f42cf1c04a60104cf7f82dde81e69d8ce` | Rust/design editor architecture, procedural vector graph, high-quality creative tooling. |
| Claude Design alternative | `github/OpenCoworkAI__open-codesign` | https://github.com/OpenCoworkAI/open-codesign.git | `2336b922cb9f67384368278bbb4b4045a5f0e5de` | Local-first Claude Design alternative; study BYOK, provider import, and artifact flow. |
| Agent development platform | `github/OpenHands__OpenHands` | https://github.com/OpenHands/OpenHands.git | `451498bdbf4d143ea8d8081461d8fde41a632a7f` | Agent runtime, sandbox, task execution, and user approval architecture. |
| AI developer | `github/Pythagora-io__gpt-pilot` | https://github.com/Pythagora-io/gpt-pilot.git | `53154df1c66b42021f230c3fb6ef797c4b7c3e83` | Long-running AI developer workflow and planning/execution split. |
| AI vector design | `github/ZSeven-W__openpencil` | https://github.com/ZSeven-W/openpencil.git | `e8ed1985b94ba954c22441a68539ef3cd3be8e6f` | AI-native vector canvas, concurrent agent teams, design-as-code reference. |
| AI coding agent | `github/aider-ai__aider` | https://github.com/aider-ai/aider.git | `3ec8ec5a7d695b08a6c24fe6c0c235c8f87df9af` | Terminal agent UX, repo edit loop, context and patch discipline. |
| Browser agent | `github/browser-use__browser-use` | https://github.com/browser-use/browser-use.git | `d19ec6ef20e5c68bf4ca198e8b0b5aea69280fe6` | Browser automation abstraction for AI-operable verification and product actions. |
| Browser agent SDK | `github/browserbase__stagehand` | https://github.com/browserbase/stagehand.git | `9ff70dd26cf4e03dce00ddcdc2d3b5e8d116781c` | Agent-friendly browser action SDK and evaluation patterns. |
| IDE agent | `github/cline__cline` | https://github.com/cline/cline.git | `ee1d4b4dcf6a8d8748edc624391faf91fa2e6d41` | IDE agent architecture, tool approval, file edits, browser/tool integration. |
| IDE agent | `github/continuedev__continue` | https://github.com/continuedev/continue.git | `cb273098d968906d25ee737b454f0b5f13ea2482` | Model/provider abstraction, IDE integration, context packaging. |
| Remotion video editor | `github/designcombo__react-video-editor` | https://github.com/designcombo/react-video-editor.git | `e2469bbffff8c5581e85f601d29fa77eab90468b` | Remotion-based timeline/editor reference for Capybara video surfaces. |
| Local AI app builder | `github/dyad-sh__dyad` | https://github.com/dyad-sh/dyad.git | `b058b83f6eef3ef1ac33a4b9f1cc0b7ef4c0c46f` | Local Lovable/Bolt alternative; study project shell, preview, and BYOK flow. |
| AI artifact template | `github/e2b-dev__fragments` | https://github.com/e2b-dev/fragments.git | `b5e627dc89b96ca0d9580bcd5ce3d7afba5f3290` | Sandboxed generated app/artifact preview reference. |
| Canvas behavior | `github/excalidraw__excalidraw` | https://github.com/excalidraw/excalidraw.git | `278cd357724b17e1119b6c76416520c42958d0e3` | Primary behavior reference for whiteboard UX, selection, drawing, and local canvas parity. |
| Lovable alternative | `github/firecrawl__open-lovable` | https://github.com/firecrawl/open-lovable.git | `69bd93bae7a9c97ef989eb70aabe6797fb3dac89` | Website-to-React and prompt-to-app pipeline reference. |
| HTML-to-video | `github/heygen-com__hyperframes` | https://github.com/heygen-com/hyperframes.git | `3f6907e807b70af23162baee9e4f5bffb41c407e` | Agent-friendly deterministic HTML video renderer; priority reference for Capybara timeline/video. |
| Creative engine | `github/invoke-ai__InvokeAI` | https://github.com/invoke-ai/InvokeAI.git | `eac4f47d0842b9f370a2e5aa4e22d0b3c40da562` | Professional image generation app/provider boundary and creative workflow reference. |
| Agent workflow platform | `github/langgenius__dify` | https://github.com/langgenius/dify.git | `3b1458c08f6a112ec7e3b24a87aad69ef4b98178` | Production AI workflow/product architecture reference. |
| Agent workspace | `github/lobehub__lobehub` | https://github.com/lobehub/lobehub.git | `1324b67590f7155441992885d301ea47f104ebb4` | Agent teammate workspace, model/provider UX, and plugin ecosystem reference. |
| Video utility | `github/mifi__lossless-cut` | https://github.com/mifi/lossless-cut.git | `260426e3d874236708ec9becf158fcdf4fd7449a` | Video/audio cut UX, media metadata, and export flow reference. |
| Code video | `github/motion-canvas__motion-canvas` | https://github.com/motion-canvas/motion-canvas.git | `00639cd4cca76d60275d48fa15211d1e17e78228` | Programmatic animation and timeline concepts. |
| Workflow automation | `github/n8n-io__n8n` | https://github.com/n8n-io/n8n.git | `2a0e2fb47ae1d82cd2354db8c2013ea46f24f21e` | Mature workflow automation, node registry, credential boundaries. |
| Claude Design alternative | `github/nexu-io__open-design` | https://github.com/nexu-io/open-design.git | `751c9de56dcc77f580fd19574c8cccc04caa5f8b` | Local-first design skills, brand systems, preview, HTML/PDF/PPTX export. |
| Video editor | `github/olive-editor__olive` | https://github.com/olive-editor/olive.git | `7e0e94abf6610026aebb9ddce8564c39522fac6e` | Non-linear video editor architecture and timeline model reference. |
| AI design/code editor | `github/onlook-dev__onlook` | https://github.com/onlook-dev/onlook.git | `a242be584fa9c71ca5be9e5e7a2640595c4200be` | Visual edit existing React app with AI; useful for inspect-and-edit UX. |
| Local AI UI | `github/open-webui__open-webui` | https://github.com/open-webui/open-webui.git | `8dae237a0bfdac4b7f55b463b3e2769ea4b94a0b` | Local AI workspace, model connections, settings, and user-facing AI app structure. |
| Design tool | `github/penpot__penpot` | https://github.com/penpot/penpot.git | `fc414b23d21cf88416799610a8308166acdfca2e` | Open design tool data model and design-code collaboration reference. |
| Visual builder | `github/plasmicapp__plasmic` | https://github.com/plasmicapp/plasmic.git | `d69a7fd7205090d4690c7afdd434a08f3e4cb53e` | Mature React visual builder and codebase integration reference. |
| React visual editor | `github/puckeditor__puck` | https://github.com/puckeditor/puck.git | `26cf9e0b872f66364a1e44533c678aefccd63ec7` | Clean React visual editor architecture, component config, and editor state. |
| Code video | `github/redotvideo__revideo` | https://github.com/redotvideo/revideo.git | `eef799d3999b1ec441a778e7f573f288a719c647` | Code-driven video alternative/reference for motion rendering. |
| Programmatic video | `github/remotion-dev__remotion` | https://github.com/remotion-dev/remotion.git | `d2113a11eb3678ab1a0ed31d593122f190b619fb` | Primary React video programming reference; license review required before reuse. |
| Node editor | `github/retejs__rete` | https://github.com/retejs/rete.git | `2aae19950180dc12725306f06c0440f64473bd21` | Visual programming editor primitives and node graph architecture. |
| Coding agent | `github/sst__opencode` | https://github.com/sst/opencode.git | `9052e8a1bac3a546c3bd06eb2f550fa8cea3b4fa` | Terminal/agent architecture, model routing, local tool loop. |
| AI app builder | `github/stackblitz__bolt.new` | https://github.com/stackblitz/bolt.new.git | `eda10b121221b30825a4c16eec5da1fd3eb1eb99` | Prompt-run-edit-deploy reference for generated apps. |
| AI design agent | `github/superdesigndev__superdesign` | https://github.com/superdesigndev/superdesign.git | `6f0824138c4e8c88db5ffa8610b0dfe5904f411f` | IDE-native product design agent; study scope and artifact flow. |
| Canvas SDK | `github/tldraw__tldraw` | https://github.com/tldraw/tldraw.git | `671472a0b122b2e071db4481e42f9c1c30784ed2` | Infinite canvas SDK state, interaction, selection, and editor architecture. |
| Website builder | `github/webstudio-is__webstudio` | https://github.com/webstudio-is/webstudio.git | `6b09e9132481df9b5b1bfdc67d7fb78518705b44` | Webflow-like builder with advanced CSS and hosted/self-hosted architecture. |
| Lovart concept scan | `github/xiaoju111a__OpenLovart` | https://github.com/xiaoju111a/OpenLovart.git | `09d82324edc2d62302db2034e39661a9df0f2614` | Direct OpenLovart concept reference; inspect product flow, not assumed code-quality benchmark. |
| Graph canvas | `github/xyflow__xyflow` | https://github.com/xyflow/xyflow.git | `a58568f11bc0e1a1bdca1b3549e959e2e1ca0cdd` | High-quality graph/canvas primitives for node-based UIs. |
