use std::io::Write;
use std::path::Path;

use flate2::Compression;
use flate2::write::ZlibEncoder;
use image::RgbaImage;

use crate::{PosterError, Result};

pub struct PdfPage<'a> {
    pub width: u32,
    pub height: u32,
    pub image: &'a RgbaImage,
}

pub fn write_pdf(path: &Path, pages: &[PdfPage<'_>]) -> Result<()> {
    let mut pdf = PdfBuilder::new();
    pdf.raw(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");
    pdf.object(1, b"<< /Type /Catalog /Pages 2 0 R >>".to_vec());
    let kids = page_ids(pages.len())
        .into_iter()
        .map(|id| format!("{id} 0 R"))
        .collect::<Vec<_>>()
        .join(" ");
    pdf.object(
        2,
        format!("<< /Type /Pages /Kids [{kids}] /Count {} >>", pages.len()).into_bytes(),
    );
    for (index, page) in pages.iter().enumerate() {
        let base = object_base(index);
        pdf.object(base, image_object(page)?);
        pdf.object(base + 1, content_object(index + 1, page.width, page.height));
        pdf.object(base + 2, page_object(base, page.width, page.height));
    }
    let bytes = pdf.finish();
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent).map_err(|source| PosterError::Write {
            path: parent.display().to_string(),
            source,
        })?;
    }
    std::fs::write(path, bytes).map_err(|source| PosterError::Write {
        path: path.display().to_string(),
        source,
    })
}

fn page_ids(count: usize) -> Vec<usize> {
    (0..count).map(|index| object_base(index) + 2).collect()
}

fn object_base(index: usize) -> usize {
    3 + index * 3
}

fn page_object(image_id: usize, width: u32, height: u32) -> Vec<u8> {
    let content_id = image_id + 1;
    format!(
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {width} {height}] /Resources << /XObject << /Im1 {image_id} 0 R >> >> /Contents {content_id} 0 R >>"
    )
    .into_bytes()
}

fn content_object(index: usize, width: u32, height: u32) -> Vec<u8> {
    let stream = format!("q\n{width} 0 0 {height} 0 0 cm\n/Im1 Do\nQ\n");
    stream_object(stream.as_bytes(), None, Some(format!("page-{index}")))
}

fn image_object(page: &PdfPage<'_>) -> Result<Vec<u8>> {
    let rgb = rgba_to_rgb(page.image);
    let compressed = deflate(&rgb)?;
    let dict = format!(
        "<< /Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /FlateDecode /Length {} >>",
        page.width,
        page.height,
        compressed.len()
    );
    Ok(stream_object(&compressed, Some(dict), None))
}

fn rgba_to_rgb(image: &RgbaImage) -> Vec<u8> {
    let mut out = Vec::with_capacity(image.width() as usize * image.height() as usize * 3);
    for pixel in image.pixels() {
        let [r, g, b, a] = pixel.0;
        let alpha = u16::from(a);
        let blend = |channel: u8| {
            let value = (u16::from(channel) * alpha + 255 * (255 - alpha)) / 255;
            u8::try_from(value).unwrap_or(255)
        };
        out.extend([blend(r), blend(g), blend(b)]);
    }
    out
}

fn deflate(bytes: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(bytes)
        .map_err(|err| PosterError::Export(format!("compress PDF image failed: {err}")))?;
    encoder
        .finish()
        .map_err(|err| PosterError::Export(format!("finish PDF compression failed: {err}")))
}

fn stream_object(bytes: &[u8], dict: Option<String>, comment: Option<String>) -> Vec<u8> {
    let dict = dict.unwrap_or_else(|| format!("<< /Length {} >>", bytes.len()));
    let mut out = Vec::new();
    if let Some(comment) = comment {
        out.extend_from_slice(format!("% {comment}\n").as_bytes());
    }
    out.extend_from_slice(dict.as_bytes());
    out.extend_from_slice(b"\nstream\n");
    out.extend_from_slice(bytes);
    out.extend_from_slice(b"\nendstream");
    out
}

struct PdfBuilder {
    data: Vec<u8>,
    offsets: Vec<usize>,
}

impl PdfBuilder {
    fn new() -> Self {
        Self {
            data: Vec::new(),
            offsets: vec![0],
        }
    }

    fn raw(&mut self, bytes: &[u8]) {
        self.data.extend_from_slice(bytes);
    }

    fn object(&mut self, id: usize, body: Vec<u8>) {
        while self.offsets.len() <= id {
            self.offsets.push(0);
        }
        self.offsets[id] = self.data.len();
        self.data
            .extend_from_slice(format!("{id} 0 obj\n").as_bytes());
        self.data.extend_from_slice(&body);
        self.data.extend_from_slice(b"\nendobj\n");
    }

    fn finish(mut self) -> Vec<u8> {
        let xref = self.data.len();
        self.data
            .extend_from_slice(format!("xref\n0 {}\n", self.offsets.len()).as_bytes());
        self.data.extend_from_slice(b"0000000000 65535 f \n");
        for offset in self.offsets.iter().skip(1) {
            self.data
                .extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
        }
        self.data.extend_from_slice(
            format!(
                "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
                self.offsets.len()
            )
            .as_bytes(),
        );
        self.data
    }
}
