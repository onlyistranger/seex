<p align="center">
  <img src="assets/icons/seex.svg" alt="SeEx icon" width="200" />
</p>

<h1 align="center">SeEx (Seek and Export)</h1>

<p align="center">
  <strong>Batch export LCSC components into formats that Altium Designer (AD), KiCad and Cadence (OrCAD Capture / Allegro) can use.</strong>
</p>

<p align="center"><a href="README.en.md">English</a> | <a href="README.md">简体中文</a></p>


---

## Before You Start
> [!NOTE]
> Although the source code is fully public, please follow the [LICENSE](LICENSE) when using this project.

The project history, in chronological order:
- At the very beginning I built `nlbn` (Nice Library Bring now, a pun on "拿来吧你"). It was open-sourced and got a fair number of stars, but it was then uploaded to `Gitee` without permission — a garbage platform, please do not upload my code there; I consider it a platform that pollutes my code. That is why I stopped maintaining `nlbn`. It also made me realise that most people do not read the `README` and do not read the `License` either, and have zero respect for copyright and other people's work; the fact that the `README` was only available in English at the time made it worse. 😢
- While developing `nlbn` I was reverse-engineering the `Altium Designer` file format, and that gradually became `npnp` (Normalize Pin Net Pad, a pun on "牛批牛批"). I tried open-sourcing it too, and it was copied anyway. Around that time I built the corresponding `GUI` version with the `tauri2` framework; it was rough, but it did solve problems for a number of people, and that made me happy. 😊
- That `GUI` tool became the current `seex` (Seek and Export). At first it called the `npnp` `cli` to do the exports, which meant maintaining two repositories at once, and over time that became tiring. So it was refactored: `seex` no longer depends on `npnp` but inherited its functionality directly, and the `GUI` framework is now `gpui-kit` (GPUI), with `egui/eframe` removed. `npnp` will keep being maintained as well, because some users need it for `CI`. 😁
- If there is a feature you want, please get in touch. My email: `lampmud@163.com` / `ybinlew@gmail.com`. You can also open an `issue`. 😊
- Please stop rewriting my tools with glue languages such as `python`. `python` is not my `taste`, thank you for understanding — especially the people who announce that `AI` finished it in two minutes. 🤣
- The project will stay on `Rust` for the long term. If the language ever changes, the only likely candidate is `Jai`, even though it has no public release yet. 😎
- When this project started there was almost nothing like it on the market; lately there are many similar projects, which is a good thing — at least everyone has something to do. But a tool like this really is not worth reskinning and putting up for sale on some platform; that is genuinely tasteless. The point of a basic tool is that every developer can use it, not to get clever about making money. Really tasteless. 🤣

## What About SeEx's Sister Project?

- [npnp](https://github.com/ref42/npnp) is the `cli` version of `seex`, and is better suited to being called directly by `AI`/agents.
  - You can even have `AI` analyse a project for you, and then call `npnp` to export the components it needs. That way you no longer have to search and pick parts by hand — you only have to verify the result.
  - If you already have the part numbers to export, even better: feed them straight to `npnp`.
  - You can also use `npnp` as a `CI` tool.

## What Can SeEx Do?
### KiCad
- [x] Export `Symbol`, `Footprint` and `3D` model formats that KiCad can use.
- [x] Export those formats in batches or one at a time.
### Altium Designer (AD)
- [x] Export a component's `schlib` and `pcblib` on its own (`pcblib` already embeds the `step` model).
- [x] Export in batches as well.
- [x] Export several components into one `schlib`/`pcblib`, i.e. a merged export.
### Cadence (OrCAD Capture / Allegro)
- [x] Export `Capture` schematic symbols: one `Library XML` file, which `Capture` converts to an `.olb` with `File > Import > Library XML`.
- [x] Export `Allegro` footprints: one `SKILL` script per footprint plus one library script; run `skill load("<library>_allegro/<library>.il")` in the `Allegro PCB Editor` command window to generate the `.dra`, `.psm` and `.pad` files in the same folder.
- [x] Export `3D` models (`step`), with the model already mapped into the footprint at the position EasyEDA uses.
- [x] Standard packages (chip resistors and capacitors, `SOT`, `SOD`, `SOIC`, `QFN`, ...) get lands calculated per `IPC-7352` and checked against the manufacturer's; other packages keep the manufacturer's lands, with silkscreen, assembly layer and place bound generated per `IPC-7352`.
- [x] `seex` itself calls no Cadence program; the import is done by you inside `Cadence`.
### Shared
- [x] All targets support exporting metadata in Chinese or English.
- [x] All targets support multi-threaded downloads.
- [x] All targets support adding new components to an existing library previously exported by `seex`.
- [x] All targets support a custom schematic stroke and fill colour.
- [x] All targets support exporting `3D` models on their own.
- [x] The LCEDA logo (watermark) is removed from `3D` models by default and the model rebuilt; when it cannot be removed safely the original model is kept, and those components are listed in the export result.

## Which EDA Versions Does SeEx Support?
> [!NOTE]
> These are only the versions known so far; SeEx may well support more. If a library exported by `seex` cannot be opened in an older `EDA` release, please open an `issue` and compatibility will be considered.
- [x] `KiCad 9.0` ~ `Latest`
- [x] `Altium Designer 23.x` ~ `Latest`
- [x] `Cadence SPB 26.1` (OrCAD X Capture / Allegro X); the `Allegro` footprint scripts need a release that supports `SKILL` (`Allegro PCB Designer` or `OrCAD PCB Designer Professional`)

## What Does SeEx Want to Do Next?
- [ ] Support more `EDA` tools, such as `pads`.
- [ ] Keep improving the current workflow and fix more hidden `bugs`.
- [ ] Keep improving the interaction and the user experience, refine the icons, and try new design languages.

## What Kind of Contributor Is SeEx Looking For?
- [x] Has software design experience and can help maintain the software over time.
- [x] Has good taste; your suggestions would be very welcome.
- [x] "Designers" who can help redesign the software's icon, the interface layout, and so on.
- [x] If you have tokens to spare and are interested in this project, feel free to give it a go: the project does not reject `AI`, it only asks that every author on a submitted `PR` is a real living person.
- [x] Contributing code is not required — every suggestion that makes `seex` and `npnp` better will be considered, and will be thanked at the end of this file.

<table>
  <summary>SeEx GUI preview</summary>

  <table>
    <tr>
      <td><img src="imgs/seex_layout/en/1.png" width="600" /></td>
      <td><img src="imgs/seex_layout/en/2.png" width="600" /></td>
      <td><img src="imgs/seex_layout/en/3.png" width="600" /></td>
    </tr>
    <tr>
      <td><img src="imgs/seex_layout/en/4.png" width="600" /></td>
      <td><img src="imgs/seex_layout/en/5.png" width="600" /></td>
    </tr>
  </table>
</table>


### Export Examples

<table>
  <summary>KiCad symbol and footprint examples</summary>

  <table>
    <tr>
      <td><img src="imgs/showcase/kicad/sch_pcb/sch_pcb_1.png" alt="KiCad symbol and footprint example 1" width="420" /></td>
      <td><img src="imgs/showcase/kicad/sch_pcb/sch_pcb_2.png" alt="KiCad symbol and footprint example 2" width="420" /></td>
      <td><img src="imgs/showcase/kicad/sch_pcb/sch_pcb_3.png" alt="KiCad symbol and footprint example 3" width="420" /></td>
    </tr>
    <tr>
      <td><img src="imgs/showcase/kicad/sch_pcb/sch_pcb_4.png" alt="KiCad symbol and footprint example 4" width="420" /></td>
      <td><img src="imgs/showcase/kicad/sch_pcb/sch_pcb_5.png" alt="KiCad symbol and footprint example 5" width="420" /></td>
      <td><img src="imgs/showcase/kicad/sch_pcb/sch_pcb_6.png" alt="KiCad symbol and footprint example 6" width="420" /></td>
    </tr>
  </table>
</table>

<table>
  <summary>KiCad 3D model examples</summary>

  <table>
    <tr>
      <td><img src="imgs/showcase/kicad/3d_models/SOT-89-3.png" alt="SOT-89-3" width="420" /></td>
      <td><img src="imgs/showcase/kicad/3d_models/TSSOP-20.png" alt="TSSOP-20" width="420" /></td>
      <td><img src="imgs/showcase/kicad/3d_models/QFN-80.png" alt="QFN-80" width="420" /></td>
    </tr>
    <tr>
      <td><img src="imgs/showcase/kicad/3d_models/SOIC-8.png" alt="SOIC-8" width="420" /></td>
      <td><img src="imgs/showcase/kicad/3d_models/SOIC-14.png" alt="SOIC-14" width="420" /></td>
      <td><img src="imgs/showcase/kicad/3d_models/LQFP-48.png" alt="LQFP-48" width="420" /></td>
    </tr>
  </table>
</table>

<table>
  <summary>Altium SCH + 3D model examples</summary>

  <table>
    <tr>
      <td><img src="imgs/showcase/ad/sch_3d/2N2222AU.png" alt="2N2222AU" width="420" /></td>
      <td><img src="imgs/showcase/ad/sch_3d/CH32V003F4P6.png" alt="CH32V003F4P6" width="420" /></td>
      <td><img src="imgs/showcase/ad/sch_3d/ESP32-S31NRV16.png" alt="ESP32-S31NRV16" width="420" /></td>
    </tr>
    <tr>
      <td><img src="imgs/showcase/ad/sch_3d/NE555DR.png" alt="NE555DR" width="420" /></td>
      <td><img src="imgs/showcase/ad/sch_3d/SN74HC04DR.png" alt="SN74HC04DR" width="420" /></td>
      <td><img src="imgs/showcase/ad/sch_3d/STM32F103C8T6.png" alt="STM32F103C8T6" width="420" /></td>
    </tr>
  </table>
</table>

<table>
  <summary>Altium PCB examples</summary>

  <table>
    <tr>
      <td><img src="imgs/showcase/ad/pcd/sot89-3.png" alt="sot89-3" width="420" /></td>
      <td><img src="imgs/showcase/ad/pcd/tssop-20.png" alt="tssop-20" width="420" /></td>
      <td><img src="imgs/showcase/ad/pcd/qfn-80.png" alt="qfn-80" width="420" /></td>
    </tr>
    <tr>
      <td><img src="imgs/showcase/ad/pcd/soic-8.png" alt="soic-8" width="420" /></td>
      <td><img src="imgs/showcase/ad/pcd/soic-14.png" alt="soic-14" width="420" /></td>
      <td><img src="imgs/showcase/ad/pcd/lqfp-48.png" alt="lqfp-48" width="420" /></td>
    </tr>
  </table>
</table>

## References And Thanks

The entries are in no particular order; they follow the development timeline.

### References

- [`easyeda2kicad`](https://github.com/uPesy/easyeda2kicad.py)@[uPesy](https://github.com/uPesy)
  - Although it only supports `KiCad`, it gave me some of the inspiration. It is an excellent tool, and the vast majority of EasyEDA import plugins for `KiCad` on the market are built on top of it.
- `IPC-7352(2023)` https://dokumen.pub/qdownload/ipc-7352-generic-guideline-for-land-pattern-design.html.
  - Used for rebuilding the `3D` models.

### Thanks

- 菊猫
  - Gave me part of my `TOKEN`. I also paid for some of it myself, but in the end it worked out as roughly half bought and half donated. `opus 4.6` was still very good at the time, and I leaned on it heavily to implement a lot of features and lay a good foundation.
- emoji老弟@[z217-cpu](https://github.com/z217-cpu)
  - Helped me test multi-platform compatibility from the very start of the project until now; thank you very much!
- 🐏🐏@[Cloud-Lightning](https://github.com/Cloud-Lightning)
  - Has used `seex` in several projects and gave me a great deal of practical advice; thank you very much!
- c大佬@[lins-code54](https://github.com/lins-code54)
  - Provided the `Cadence` export support, and the `3D` model rebuild feature so that models no longer carry the cloud-pattern or text `Logo`; thank you very much!
- Everyone in WeChat groups `1` and `2`
  - They did not mind me posting `seex` and `npnp` updates in those groups; on the contrary, they were very supportive. Thank you very much!
- All users
  - Without your downloads and `star`s this project might have been shelved; thank you very much!
