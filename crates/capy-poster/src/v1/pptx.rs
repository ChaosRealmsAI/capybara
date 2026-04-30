use std::io::{Cursor, Write};
use std::path::Path;

use crate::{PosterError, Result};

pub struct PptxPage<'a> {
    pub title: &'a str,
    pub width: u32,
    pub height: u32,
    pub png: &'a [u8],
}

pub fn write_pptx(path: &Path, pages: &[PptxPage<'_>]) -> Result<()> {
    let mut bytes = Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(&mut bytes);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    add(
        &mut zip,
        options,
        "[Content_Types].xml",
        &content_types(pages.len()),
    )?;
    add(&mut zip, options, "_rels/.rels", ROOT_RELS)?;
    add(
        &mut zip,
        options,
        "ppt/presentation.xml",
        &presentation(pages),
    )?;
    add(
        &mut zip,
        options,
        "ppt/_rels/presentation.xml.rels",
        &presentation_rels(pages.len()),
    )?;
    add(&mut zip, options, "ppt/theme/theme1.xml", THEME)?;
    add(
        &mut zip,
        options,
        "ppt/slideMasters/slideMaster1.xml",
        SLIDE_MASTER,
    )?;
    add(
        &mut zip,
        options,
        "ppt/slideMasters/_rels/slideMaster1.xml.rels",
        SLIDE_MASTER_RELS,
    )?;
    add(
        &mut zip,
        options,
        "ppt/slideLayouts/slideLayout1.xml",
        SLIDE_LAYOUT,
    )?;
    add(
        &mut zip,
        options,
        "ppt/slideLayouts/_rels/slideLayout1.xml.rels",
        SLIDE_LAYOUT_RELS,
    )?;
    for (index, page) in pages.iter().enumerate() {
        let number = index + 1;
        add(
            &mut zip,
            options,
            &format!("ppt/slides/slide{number}.xml"),
            &slide_xml(page),
        )?;
        add(
            &mut zip,
            options,
            &format!("ppt/slides/_rels/slide{number}.xml.rels"),
            &slide_rels(number),
        )?;
        zip.start_file(format!("ppt/media/page{number}.png"), options)
            .map_err(zip_err)?;
        zip.write_all(page.png).map_err(zip_err)?;
    }
    zip.finish().map_err(zip_err)?;
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent).map_err(|source| PosterError::Write {
            path: parent.display().to_string(),
            source,
        })?;
    }
    std::fs::write(path, bytes.into_inner()).map_err(|source| PosterError::Write {
        path: path.display().to_string(),
        source,
    })
}

fn add<W: Write + std::io::Seek>(
    zip: &mut zip::ZipWriter<W>,
    options: zip::write::SimpleFileOptions,
    path: &str,
    text: &str,
) -> Result<()> {
    zip.start_file(path, options).map_err(zip_err)?;
    zip.write_all(text.as_bytes()).map_err(zip_err)
}

fn content_types(count: usize) -> String {
    let slides = (1..=count)
        .map(|index| {
            format!(r#"<Override PartName="/ppt/slides/slide{index}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>"#)
        })
        .collect::<String>();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="png" ContentType="image/png"/><Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/><Override PartName="/ppt/slideMasters/slideMaster1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"/><Override PartName="/ppt/slideLayouts/slideLayout1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/><Override PartName="/ppt/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/>{slides}</Types>"#
    )
}

fn presentation(pages: &[PptxPage<'_>]) -> String {
    let (width, height) = pages
        .first()
        .map(|page| emu_size(page))
        .unwrap_or((9144000, 5143500));
    let slides = pages
        .iter()
        .enumerate()
        .map(|(index, _)| format!(r#"<p:sldId id="{}" r:id="rId{}"/>"#, 256 + index, index + 2))
        .collect::<String>();
    format!(
        r#"<p:presentation xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:sldMasterIdLst><p:sldMasterId id="2147483648" r:id="rId1"/></p:sldMasterIdLst><p:sldIdLst>{slides}</p:sldIdLst><p:sldSz cx="{width}" cy="{height}"/><p:notesSz cx="6858000" cy="9144000"/></p:presentation>"#
    )
}

fn presentation_rels(count: usize) -> String {
    let slides = (1..=count)
        .map(|index| {
            format!(r#"<Relationship Id="rId{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide{index}.xml"/>"#, index + 1)
        })
        .collect::<String>();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="slideMasters/slideMaster1.xml"/>{slides}</Relationships>"#
    )
}

fn slide_xml(page: &PptxPage<'_>) -> String {
    let (cx, cy) = emu_size(page);
    format!(
        r#"<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cSld name="{name}"><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{cx}" cy="{cy}"/><a:chOff x="0" y="0"/><a:chExt cx="{cx}" cy="{cy}"/></a:xfrm></p:grpSpPr><p:pic><p:nvPicPr><p:cNvPr id="2" name="{name}"/><p:cNvPicPr/><p:nvPr/></p:nvPicPr><p:blipFill><a:blip r:embed="rId1"/><a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr></p:pic></p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>"#,
        name = xml_attr(page.title),
    )
}

fn slide_rels(index: usize) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/page{index}.png"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/></Relationships>"#
    )
}

fn emu_size(page: &PptxPage<'_>) -> (u64, u64) {
    (u64::from(page.width) * 9525, u64::from(page.height) * 9525)
}

fn zip_err(err: impl std::fmt::Display) -> PosterError {
    PosterError::Export(format!("write PPTX failed: {err}"))
}

fn xml_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

const ROOT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/></Relationships>"#;
const SLIDE_MASTER_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="../theme/theme1.xml"/></Relationships>"#;
const SLIDE_LAYOUT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="../slideMasters/slideMaster1.xml"/></Relationships>"#;
const SLIDE_MASTER: &str = r#"<p:sldMaster xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/></p:spTree></p:cSld><p:sldLayoutIdLst><p:sldLayoutId id="2147483649" r:id="rId1"/></p:sldLayoutIdLst><p:txStyles/></p:sldMaster>"#;
const SLIDE_LAYOUT: &str = r#"<p:sldLayout xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" type="blank"><p:cSld name="Blank"><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/></p:spTree></p:cSld></p:sldLayout>"#;
const THEME: &str = r#"<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="Capybara"><a:themeElements><a:clrScheme name="Capybara"><a:dk1><a:srgbClr val="1C1917"/></a:dk1><a:lt1><a:srgbClr val="FFFAF0"/></a:lt1><a:dk2><a:srgbClr val="57534E"/></a:dk2><a:lt2><a:srgbClr val="FEF3C7"/></a:lt2><a:accent1><a:srgbClr val="A78BFA"/></a:accent1><a:accent2><a:srgbClr val="84CC16"/></a:accent2><a:accent3><a:srgbClr val="F59E0B"/></a:accent3><a:accent4><a:srgbClr val="FB7185"/></a:accent4><a:accent5><a:srgbClr val="60A5FA"/></a:accent5><a:accent6><a:srgbClr val="34D399"/></a:accent6><a:hlink><a:srgbClr val="5B8ABF"/></a:hlink><a:folHlink><a:srgbClr val="8A6FAE"/></a:folHlink></a:clrScheme><a:fontScheme name="Capybara"><a:majorFont><a:latin typeface="Arial"/></a:majorFont><a:minorFont><a:latin typeface="Arial"/></a:minorFont></a:fontScheme><a:fmtScheme name="Capybara"><a:fillStyleLst/><a:lnStyleLst/><a:effectStyleLst/><a:bgFillStyleLst/></a:fmtScheme></a:themeElements></a:theme>"#;
