#![forbid(unsafe_code)]

use std::{error::Error as StdError, fmt};

use avif_serialize::{
    Aviffy,
    constants::{
        ColorPrimaries as AvifColorPrimaries, MatrixCoefficients as AvifMatrixCoefficients,
        TransferCharacteristics as AvifTransferCharacteristics,
    },
};
use rav1e::prelude::{
    ChromaSampling, ChromaticityPoint, ColorDescription, ColorPrimaries, Config, ContentLight,
    Context, EncoderConfig, EncoderStatus, Frame, MasteringDisplay, MatrixCoefficients, PixelRange,
    Plane, TransferCharacteristics,
};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    InvalidImageBuffer { width: u32, height: u32, len: usize },
    InvalidOption(&'static str),
    Unsupported(&'static str),
    Encoder(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidImageBuffer { width, height, len } => write!(
                f,
                "pixel buffer length {len} does not match {width}x{height} image size"
            ),
            Self::InvalidOption(message) | Self::Unsupported(message) => f.write_str(message),
            Self::Encoder(message) => f.write_str(message),
        }
    }
}

impl StdError for Error {}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Chromaticity {
    pub x: f32,
    pub y: f32,
}

impl Chromaticity {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MasteringDisplayMetadata {
    pub red: Chromaticity,
    pub green: Chromaticity,
    pub blue: Chromaticity,
    pub white_point: Chromaticity,
    pub max_luminance_nits: f32,
    pub min_luminance_nits: f32,
}

impl MasteringDisplayMetadata {
    pub const fn bt2020_pq_1000_nits() -> Self {
        Self {
            red: Chromaticity::new(0.708, 0.292),
            green: Chromaticity::new(0.170, 0.797),
            blue: Chromaticity::new(0.131, 0.046),
            white_point: Chromaticity::new(0.3127, 0.3290),
            max_luminance_nits: 1000.0,
            min_luminance_nits: 0.005,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContentLightMetadata {
    pub max_content_light_level: u16,
    pub max_frame_average_light_level: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NclxProfile {
    pub color_primaries: ColorPrimaries,
    pub transfer_characteristics: TransferCharacteristics,
    pub matrix_coefficients: MatrixCoefficients,
    pub pixel_range: PixelRange,
}

impl NclxProfile {
    pub const fn bt2020_pq_full_range() -> Self {
        Self {
            color_primaries: ColorPrimaries::BT2020,
            transfer_characteristics: TransferCharacteristics::SMPTE2084,
            matrix_coefficients: MatrixCoefficients::BT2020NCL,
            pixel_range: PixelRange::Full,
        }
    }

    pub const fn bt709_srgb_full_range() -> Self {
        Self {
            color_primaries: ColorPrimaries::BT709,
            transfer_characteristics: TransferCharacteristics::SRGB,
            matrix_coefficients: MatrixCoefficients::BT709,
            pixel_range: PixelRange::Full,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct HdrEncodingOptions {
    pub bit_depth: u8,
    pub nclx_profile: NclxProfile,
    pub source_white_level: f32,
    pub reference_white_nits: f32,
    pub mastering_display: Option<MasteringDisplayMetadata>,
    pub content_light: Option<ContentLightMetadata>,
}

impl Default for HdrEncodingOptions {
    fn default() -> Self {
        Self {
            bit_depth: 10,
            nclx_profile: NclxProfile::bt2020_pq_full_range(),
            source_white_level: 1.0,
            reference_white_nits: 203.0,
            mastering_display: Some(MasteringDisplayMetadata::bt2020_pq_1000_nits()),
            content_light: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SdrEncodingOptions {
    pub bit_depth: u8,
    pub nclx_profile: NclxProfile,
}

impl Default for SdrEncodingOptions {
    fn default() -> Self {
        Self {
            bit_depth: 10,
            nclx_profile: NclxProfile::bt709_srgb_full_range(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RgbaF32Image<'a> {
    width: u32,
    height: u32,
    pixels: &'a [f32],
}

impl<'a> RgbaF32Image<'a> {
    pub fn new(width: u32, height: u32, pixels: &'a [f32]) -> Result<Self> {
        validate_len(width, height, pixels.len())?;
        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }

    pub const fn pixels(&self) -> &'a [f32] {
        self.pixels
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RgbaU8Image<'a> {
    width: u32,
    height: u32,
    pixels: &'a [u8],
}

impl<'a> RgbaU8Image<'a> {
    pub fn new(width: u32, height: u32, pixels: &'a [u8]) -> Result<Self> {
        validate_len(width, height, pixels.len())?;
        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }

    pub const fn pixels(&self) -> &'a [u8] {
        self.pixels
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedAvif {
    pub avif_file: Vec<u8>,
    pub color_byte_size: usize,
    pub alpha_byte_size: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Encoder {
    quality: f32,
    alpha_quality: f32,
    speed: u8,
    threads: Option<usize>,
}

impl Default for Encoder {
    fn default() -> Self {
        Self::new()
    }
}

impl Encoder {
    pub fn new() -> Self {
        Self {
            quality: 80.0,
            alpha_quality: 80.0,
            speed: 4,
            threads: None,
        }
    }

    pub fn with_quality(mut self, quality: f32) -> Self {
        self.quality = quality;
        self
    }

    pub fn with_alpha_quality(mut self, quality: f32) -> Self {
        self.alpha_quality = quality;
        self
    }

    pub fn with_speed(mut self, speed: u8) -> Self {
        self.speed = speed;
        self
    }

    pub fn with_num_threads(mut self, threads: Option<usize>) -> Self {
        self.threads = threads;
        self
    }

    pub fn encode_hdr_rgba_f32(
        &self,
        image: &RgbaF32Image<'_>,
        options: &HdrEncodingOptions,
    ) -> Result<EncodedAvif> {
        validate_common_encoder_settings(self.speed, self.quality, self.alpha_quality)?;
        validate_hdr_options(options)?;

        let transform = build_hdr_transform(options);
        let converted = convert_hdr_image_to_planes(image, &transform, options.bit_depth);
        let content_light = options
            .content_light
            .unwrap_or_else(|| converted.derived_content_light(transform.peak_luminance_nits));

        encode_prepared(
            self,
            PreparedImage {
                descriptor: ImageDescriptor {
                    width: image.width,
                    height: image.height,
                    bit_depth: options.bit_depth,
                    nclx_profile: options.nclx_profile,
                    mastering_display: options.mastering_display,
                    content_light: Some(content_light),
                },
                converted,
            },
        )
    }

    pub fn encode_sdr_rgba8(
        &self,
        image: &RgbaU8Image<'_>,
        options: &SdrEncodingOptions,
    ) -> Result<EncodedAvif> {
        validate_common_encoder_settings(self.speed, self.quality, self.alpha_quality)?;
        validate_sdr_options(options)?;

        let converted = convert_sdr_image_to_planes(image, options.bit_depth, options.nclx_profile);
        encode_prepared(
            self,
            PreparedImage {
                descriptor: ImageDescriptor {
                    width: image.width,
                    height: image.height,
                    bit_depth: options.bit_depth,
                    nclx_profile: options.nclx_profile,
                    mastering_display: None,
                    content_light: None,
                },
                converted,
            },
        )
    }
}

#[derive(Debug, Clone, Copy)]
struct HdrTransform {
    source_white_level: f32,
    reference_white_nits: f32,
    peak_luminance_nits: f32,
    luma_coefficients: [f32; 3],
}

#[derive(Debug, Clone)]
struct ConvertedImage {
    y: Vec<u16>,
    u: Vec<u16>,
    v: Vec<u16>,
    alpha: Vec<u16>,
    has_alpha: bool,
    max_content_light_level: u16,
    max_frame_average_light_level: u16,
}

#[derive(Debug, Clone, Copy)]
struct ImageDescriptor {
    width: u32,
    height: u32,
    bit_depth: u8,
    nclx_profile: NclxProfile,
    mastering_display: Option<MasteringDisplayMetadata>,
    content_light: Option<ContentLightMetadata>,
}

#[derive(Debug, Clone)]
struct PreparedImage {
    descriptor: ImageDescriptor,
    converted: ConvertedImage,
}

impl ConvertedImage {
    fn derived_content_light(&self, _fallback_peak: f32) -> ContentLightMetadata {
        ContentLightMetadata {
            max_content_light_level: self.max_content_light_level,
            max_frame_average_light_level: self.max_frame_average_light_level,
        }
    }
}

fn validate_len(width: u32, height: u32, len: usize) -> Result<()> {
    let expected_len = width as usize * height as usize * 4;
    if len != expected_len {
        return Err(Error::InvalidImageBuffer { width, height, len });
    }
    Ok(())
}

fn validate_common_encoder_settings(speed: u8, quality: f32, alpha_quality: f32) -> Result<()> {
    if !(1..=10).contains(&speed) {
        return Err(Error::InvalidOption("speed must be between 1 and 10"));
    }
    if !(1.0..=100.0).contains(&quality) || !(1.0..=100.0).contains(&alpha_quality) {
        return Err(Error::InvalidOption("quality must be between 1 and 100"));
    }
    Ok(())
}

fn validate_hdr_options(options: &HdrEncodingOptions) -> Result<()> {
    if options.bit_depth != 10 && options.bit_depth != 12 {
        return Err(Error::InvalidOption(
            "HDR AVIF encoding currently supports 10-bit or 12-bit output",
        ));
    }
    if !options.source_white_level.is_finite() || options.source_white_level <= 0.0 {
        return Err(Error::InvalidOption(
            "source_white_level must be a finite positive value",
        ));
    }
    if !options.reference_white_nits.is_finite() || options.reference_white_nits <= 0.0 {
        return Err(Error::InvalidOption(
            "reference_white_nits must be a finite positive value",
        ));
    }
    validate_profile(options.nclx_profile, true)
}

fn validate_sdr_options(options: &SdrEncodingOptions) -> Result<()> {
    if options.bit_depth != 8 && options.bit_depth != 10 && options.bit_depth != 12 {
        return Err(Error::InvalidOption(
            "SDR AVIF encoding supports 8-bit, 10-bit, or 12-bit output",
        ));
    }
    validate_profile(options.nclx_profile, false)
}

fn validate_profile(profile: NclxProfile, hdr: bool) -> Result<()> {
    match profile.matrix_coefficients {
        MatrixCoefficients::BT709
        | MatrixCoefficients::BT2020NCL
        | MatrixCoefficients::Identity => {}
        _ => {
            return Err(Error::Unsupported(
                "only BT709, BT2020NCL, and Identity matrix coefficients are currently supported",
            ));
        }
    }
    if profile.pixel_range != PixelRange::Full {
        return Err(Error::Unsupported(
            "AVIF encoding currently supports full-range signaling only",
        ));
    }
    if hdr {
        if profile.transfer_characteristics != TransferCharacteristics::SMPTE2084 {
            return Err(Error::Unsupported(
                "HDR AVIF encoding currently supports SMPTE2084 (PQ) transfer only",
            ));
        }
    } else if profile.transfer_characteristics != TransferCharacteristics::SRGB {
        return Err(Error::Unsupported(
            "SDR AVIF encoding currently supports sRGB transfer only",
        ));
    }
    Ok(())
}

fn build_hdr_transform(options: &HdrEncodingOptions) -> HdrTransform {
    let peak_luminance_nits = options
        .mastering_display
        .map(|metadata| metadata.max_luminance_nits)
        .unwrap_or(1000.0)
        .max(options.reference_white_nits)
        .max(1.0);

    HdrTransform {
        source_white_level: options.source_white_level,
        reference_white_nits: options.reference_white_nits,
        peak_luminance_nits,
        luma_coefficients: luma_coefficients(options.nclx_profile.matrix_coefficients),
    }
}

fn luma_coefficients(matrix: MatrixCoefficients) -> [f32; 3] {
    match matrix {
        MatrixCoefficients::Identity => [0.0, 1.0, 0.0],
        MatrixCoefficients::BT709 => [0.2126, 0.7152, 0.0722],
        MatrixCoefficients::BT2020NCL => [0.2627, 0.6780, 0.0593],
        _ => unreachable!(),
    }
}

fn convert_hdr_image_to_planes(
    image: &RgbaF32Image<'_>,
    transform: &HdrTransform,
    bit_depth: u8,
) -> ConvertedImage {
    let max_code = ((1u32 << bit_depth) - 1) as f32;
    let mut y = Vec::with_capacity((image.width * image.height) as usize);
    let mut u = Vec::with_capacity((image.width * image.height) as usize);
    let mut v = Vec::with_capacity((image.width * image.height) as usize);
    let mut alpha = Vec::with_capacity((image.width * image.height) as usize);
    let mut has_alpha = false;
    let mut max_content_light_level = 0.0f32;
    let mut total_luminance = 0.0f32;
    let mut pixel_count = 0usize;

    for row in image.pixels.chunks_exact(image.width as usize * 4) {
        for rgba in row.chunks_exact(4) {
            let linear_rgb = [rgba[0].max(0.0), rgba[1].max(0.0), rgba[2].max(0.0)];
            let absolute_rgb = linear_rgb.map(|channel| {
                (channel / transform.source_white_level * transform.reference_white_nits)
                    .clamp(0.0, transform.peak_luminance_nits)
            });
            let encoded_rgb = absolute_rgb.map(pq_encode_absolute_nits);
            let [kr, kg, kb] = transform.luma_coefficients;
            let (plane_y, plane_u, plane_v) = encoded_rgb_to_planes(encoded_rgb, kr, kg, kb);
            y.push(quantize_unit_interval(plane_y, max_code));
            u.push(quantize_unit_interval(plane_u, max_code));
            v.push(quantize_unit_interval(plane_v, max_code));

            let alpha_code = quantize_unit_interval(rgba[3].clamp(0.0, 1.0), max_code);
            has_alpha |= alpha_code < max_code as u16;
            alpha.push(alpha_code);

            let pixel_max = absolute_rgb[0].max(absolute_rgb[1]).max(absolute_rgb[2]);
            max_content_light_level = max_content_light_level.max(pixel_max);
            total_luminance += absolute_rgb[0] * kr + absolute_rgb[1] * kg + absolute_rgb[2] * kb;
            pixel_count += 1;
        }
    }

    let frame_average_luminance = if pixel_count == 0 {
        0.0
    } else {
        total_luminance / pixel_count as f32
    };

    ConvertedImage {
        y,
        u,
        v,
        alpha,
        has_alpha,
        max_content_light_level: max_content_light_level.round().clamp(0.0, u16::MAX as f32) as u16,
        max_frame_average_light_level: frame_average_luminance.round().clamp(0.0, u16::MAX as f32)
            as u16,
    }
}

fn convert_sdr_image_to_planes(
    image: &RgbaU8Image<'_>,
    bit_depth: u8,
    profile: NclxProfile,
) -> ConvertedImage {
    let max_code = ((1u32 << bit_depth) - 1) as f32;
    let [kr, kg, kb] = luma_coefficients(profile.matrix_coefficients);
    let mut y = Vec::with_capacity((image.width * image.height) as usize);
    let mut u = Vec::with_capacity((image.width * image.height) as usize);
    let mut v = Vec::with_capacity((image.width * image.height) as usize);
    let mut alpha = Vec::with_capacity((image.width * image.height) as usize);
    let mut has_alpha = false;

    for rgba in image.pixels.chunks_exact(4) {
        let encoded_rgb = [
            rgba[0] as f32 / 255.0,
            rgba[1] as f32 / 255.0,
            rgba[2] as f32 / 255.0,
        ];
        let (plane_y, plane_u, plane_v) = encoded_rgb_to_planes(encoded_rgb, kr, kg, kb);
        y.push(quantize_unit_interval(plane_y, max_code));
        u.push(quantize_unit_interval(plane_u, max_code));
        v.push(quantize_unit_interval(plane_v, max_code));

        let alpha_code = quantize_unit_interval(rgba[3] as f32 / 255.0, max_code);
        has_alpha |= rgba[3] != 255;
        alpha.push(alpha_code);
    }

    ConvertedImage {
        y,
        u,
        v,
        alpha,
        has_alpha,
        max_content_light_level: 0,
        max_frame_average_light_level: 0,
    }
}

fn encoded_rgb_to_planes(rgb: [f32; 3], kr: f32, kg: f32, kb: f32) -> (f32, f32, f32) {
    if kr == 0.0 && kg == 1.0 && kb == 0.0 {
        (rgb[1], rgb[2], rgb[0])
    } else {
        let y = kr * rgb[0] + kg * rgb[1] + kb * rgb[2];
        let cb = 0.5 + 0.5 * (rgb[2] - y) / (1.0 - kb);
        let cr = 0.5 + 0.5 * (rgb[0] - y) / (1.0 - kr);
        (y, cb, cr)
    }
}

fn encode_prepared(encoder: &Encoder, prepared: PreparedImage) -> Result<EncodedAvif> {
    let descriptor = prepared.descriptor;
    let color_payload = encode_color_payload(encoder, descriptor, &prepared.converted)?;
    let alpha_payload = if prepared.converted.has_alpha {
        Some(encode_alpha_payload(
            encoder,
            descriptor.width,
            descriptor.height,
            descriptor.bit_depth,
            &prepared.converted.alpha,
        )?)
    } else {
        None
    };

    let avif_file = mux_avif(descriptor, &color_payload, alpha_payload.as_deref())?;

    Ok(EncodedAvif {
        avif_file,
        color_byte_size: color_payload.len(),
        alpha_byte_size: alpha_payload.as_ref().map_or(0, Vec::len),
    })
}

fn encode_color_payload(
    encoder: &Encoder,
    descriptor: ImageDescriptor,
    converted: &ConvertedImage,
) -> Result<Vec<u8>> {
    let mut config = EncoderConfig::with_speed_preset(encoder.speed);
    config.width = descriptor.width as usize;
    config.height = descriptor.height as usize;
    config.bit_depth = descriptor.bit_depth as usize;
    config.chroma_sampling = ChromaSampling::Cs444;
    config.pixel_range = descriptor.nclx_profile.pixel_range;
    config.color_description = Some(ColorDescription {
        color_primaries: descriptor.nclx_profile.color_primaries,
        transfer_characteristics: descriptor.nclx_profile.transfer_characteristics,
        matrix_coefficients: descriptor.nclx_profile.matrix_coefficients,
    });
    config.mastering_display = descriptor.mastering_display.map(to_rav1e_mastering_display);
    config.content_light = descriptor.content_light.map(|metadata| ContentLight {
        max_content_light_level: metadata.max_content_light_level,
        max_frame_average_light_level: metadata.max_frame_average_light_level,
    });
    config.still_picture = true;
    config.low_latency = true;
    config.min_key_frame_interval = 1;
    config.max_key_frame_interval = 1;
    config.quantizer = quality_to_quantizer(encoder.quality) as usize;

    let cfg = Config::new()
        .with_encoder_config(config)
        .with_threads(encoder.threads.unwrap_or(0));
    let context: Context<u16> = cfg
        .new_context()
        .map_err(|error| Error::Encoder(error.to_string()))?;
    let mut frame: Frame<u16> = context.new_frame();

    copy_plane(
        &mut frame.planes[0],
        &converted.y,
        descriptor.width as usize,
        descriptor.height as usize,
    );
    copy_plane(
        &mut frame.planes[1],
        &converted.u,
        descriptor.width as usize,
        descriptor.height as usize,
    );
    copy_plane(
        &mut frame.planes[2],
        &converted.v,
        descriptor.width as usize,
        descriptor.height as usize,
    );

    encode_single_frame(context, frame)
}

fn encode_alpha_payload(
    encoder: &Encoder,
    width: u32,
    height: u32,
    bit_depth: u8,
    alpha: &[u16],
) -> Result<Vec<u8>> {
    let mut config = EncoderConfig::with_speed_preset(encoder.speed);
    config.width = width as usize;
    config.height = height as usize;
    config.bit_depth = bit_depth as usize;
    config.chroma_sampling = ChromaSampling::Cs400;
    config.pixel_range = PixelRange::Full;
    config.color_description = None;
    config.mastering_display = None;
    config.content_light = None;
    config.still_picture = true;
    config.low_latency = true;
    config.min_key_frame_interval = 1;
    config.max_key_frame_interval = 1;
    config.quantizer = quality_to_quantizer(encoder.alpha_quality) as usize;

    let cfg = Config::new()
        .with_encoder_config(config)
        .with_threads(encoder.threads.unwrap_or(0));
    let context: Context<u16> = cfg
        .new_context()
        .map_err(|error| Error::Encoder(error.to_string()))?;
    let mut frame: Frame<u16> = context.new_frame();
    copy_plane(&mut frame.planes[0], alpha, width as usize, height as usize);

    encode_single_frame(context, frame)
}

fn encode_single_frame(mut context: Context<u16>, frame: Frame<u16>) -> Result<Vec<u8>> {
    context
        .send_frame(frame)
        .map_err(|error| Error::Encoder(error.to_string()))?;
    context.flush();

    let mut payload = Vec::new();
    loop {
        match context.receive_packet() {
            Ok(packet) => payload.extend_from_slice(&packet.data),
            Err(EncoderStatus::Encoded) => {}
            Err(EncoderStatus::LimitReached) => break,
            Err(error) => return Err(Error::Encoder(error.to_string())),
        }
    }

    if payload.is_empty() {
        return Err(Error::Encoder(
            "rav1e did not emit an AV1 payload for the still image".to_string(),
        ));
    }

    Ok(payload)
}

fn copy_plane(plane: &mut Plane<u16>, source: &[u16], width: usize, height: usize) {
    let stride = plane.cfg.stride;
    let data = plane.data_origin_mut();
    for row_index in 0..height {
        let src_start = row_index * width;
        let src_end = src_start + width;
        let dst_start = row_index * stride;
        let dst_end = dst_start + width;
        data[dst_start..dst_end].copy_from_slice(&source[src_start..src_end]);
    }
}

fn mux_avif(
    descriptor: ImageDescriptor,
    color_payload: &[u8],
    alpha_payload: Option<&[u8]>,
) -> Result<Vec<u8>> {
    let mut avif = Aviffy::new();
    avif.set_width(descriptor.width)
        .set_height(descriptor.height)
        .set_bit_depth(descriptor.bit_depth)
        .set_full_color_range(descriptor.nclx_profile.pixel_range == PixelRange::Full)
        .set_color_primaries(map_color_primaries(
            descriptor.nclx_profile.color_primaries,
        )?)
        .set_transfer_characteristics(map_transfer(
            descriptor.nclx_profile.transfer_characteristics,
        )?)
        .set_matrix_coefficients(map_matrix(descriptor.nclx_profile.matrix_coefficients)?);

    if let Some(metadata) = descriptor.content_light {
        avif.set_content_light_level(
            metadata.max_content_light_level,
            metadata.max_frame_average_light_level,
        );
    }

    if let Some(metadata) = descriptor.mastering_display {
        avif.set_mastering_display(
            [
                to_avif_xy(metadata.green),
                to_avif_xy(metadata.blue),
                to_avif_xy(metadata.red),
            ],
            to_avif_xy(metadata.white_point),
            (metadata.max_luminance_nits * 10_000.0)
                .round()
                .clamp(0.0, u32::MAX as f32) as u32,
            (metadata.min_luminance_nits * 10_000.0)
                .round()
                .clamp(0.0, u32::MAX as f32) as u32,
        );
    }

    Ok(avif.to_vec(
        color_payload,
        alpha_payload,
        descriptor.width,
        descriptor.height,
        descriptor.bit_depth,
    ))
}

fn to_rav1e_mastering_display(metadata: MasteringDisplayMetadata) -> MasteringDisplay {
    MasteringDisplay {
        primaries: [
            to_rav1e_point(metadata.red),
            to_rav1e_point(metadata.green),
            to_rav1e_point(metadata.blue),
        ],
        white_point: to_rav1e_point(metadata.white_point),
        max_luminance: (metadata.max_luminance_nits * 256.0)
            .round()
            .clamp(0.0, u32::MAX as f32) as u32,
        min_luminance: (metadata.min_luminance_nits * 16_384.0)
            .round()
            .clamp(0.0, u32::MAX as f32) as u32,
    }
}

fn to_rav1e_point(point: Chromaticity) -> ChromaticityPoint {
    ChromaticityPoint {
        x: (point.x.clamp(0.0, 1.0) * 65_536.0).round() as u16,
        y: (point.y.clamp(0.0, 1.0) * 65_536.0).round() as u16,
    }
}

fn to_avif_xy(point: Chromaticity) -> (u16, u16) {
    (
        (point.x.clamp(0.0, 1.0) * 50_000.0).round() as u16,
        (point.y.clamp(0.0, 1.0) * 50_000.0).round() as u16,
    )
}

fn map_color_primaries(primaries: ColorPrimaries) -> Result<AvifColorPrimaries> {
    match primaries {
        ColorPrimaries::BT709 => Ok(AvifColorPrimaries::Bt709),
        ColorPrimaries::Unspecified => Ok(AvifColorPrimaries::Unspecified),
        ColorPrimaries::BT2020 => Ok(AvifColorPrimaries::Bt2020),
        ColorPrimaries::SMPTE431 => Ok(AvifColorPrimaries::DciP3),
        ColorPrimaries::SMPTE432 => Ok(AvifColorPrimaries::DisplayP3),
        _ => Err(Error::Unsupported("unsupported AVIF color primaries")),
    }
}

fn map_transfer(transfer: TransferCharacteristics) -> Result<AvifTransferCharacteristics> {
    match transfer {
        TransferCharacteristics::BT709 => Ok(AvifTransferCharacteristics::Bt709),
        TransferCharacteristics::Unspecified => Ok(AvifTransferCharacteristics::Unspecified),
        TransferCharacteristics::Linear => Ok(AvifTransferCharacteristics::Linear),
        TransferCharacteristics::SRGB => Ok(AvifTransferCharacteristics::Srgb),
        TransferCharacteristics::BT2020_10Bit => Ok(AvifTransferCharacteristics::Bt2020_10),
        TransferCharacteristics::BT2020_12Bit => Ok(AvifTransferCharacteristics::Bt2020_12),
        TransferCharacteristics::SMPTE2084 => Ok(AvifTransferCharacteristics::Smpte2084),
        TransferCharacteristics::HLG => Ok(AvifTransferCharacteristics::Hlg),
        _ => Err(Error::Unsupported(
            "unsupported AVIF transfer characteristics",
        )),
    }
}

fn map_matrix(matrix: MatrixCoefficients) -> Result<AvifMatrixCoefficients> {
    match matrix {
        MatrixCoefficients::Identity => Ok(AvifMatrixCoefficients::Rgb),
        MatrixCoefficients::BT709 => Ok(AvifMatrixCoefficients::Bt709),
        MatrixCoefficients::Unspecified => Ok(AvifMatrixCoefficients::Unspecified),
        MatrixCoefficients::BT601 => Ok(AvifMatrixCoefficients::Bt601),
        MatrixCoefficients::YCgCo => Ok(AvifMatrixCoefficients::Ycgco),
        MatrixCoefficients::BT2020NCL => Ok(AvifMatrixCoefficients::Bt2020Ncl),
        MatrixCoefficients::BT2020CL => Ok(AvifMatrixCoefficients::Bt2020Cl),
        _ => Err(Error::Unsupported("unsupported AVIF matrix coefficients")),
    }
}

fn quality_to_quantizer(quality: f32) -> u8 {
    let q = quality / 100.0;
    let x = if q >= 0.82 {
        (1.0 - q) * 2.6
    } else if q > 0.25 {
        q.mul_add(-0.5, 0.875)
    } else {
        1.0 - q
    };
    (x * 255.0).round() as u8
}

fn quantize_unit_interval(value: f32, max_code: f32) -> u16 {
    value.clamp(0.0, 1.0).mul_add(max_code, 0.0).round() as u16
}

fn pq_encode_absolute_nits(nits: f32) -> f32 {
    const M1: f32 = 0.159_301_76;
    const M2: f32 = 78.84375;
    const C1: f32 = 0.8359375;
    const C2: f32 = 18.851562;
    const C3: f32 = 18.6875;

    let luminance = (nits / 10_000.0).clamp(0.0, 1.0);
    if luminance <= 0.0 {
        return 0.0;
    }

    let luminance_power = luminance.powf(M1);
    ((C1 + C2 * luminance_power) / (1.0 + C3 * luminance_power)).powf(M2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgba_f32_image_rejects_wrong_buffer_length() {
        let result = RgbaF32Image::new(2, 1, &[0.0, 0.0, 0.0, 1.0]);
        assert!(result.is_err());
    }

    #[test]
    fn encode_sdr_rgba8_produces_avif_payload() {
        let image = RgbaU8Image::new(2, 1, &[255, 0, 0, 255, 0, 255, 0, 255]).unwrap();
        let encoded = Encoder::new()
            .encode_sdr_rgba8(&image, &SdrEncodingOptions::default())
            .unwrap();

        assert!(!encoded.avif_file.is_empty());
        assert!(encoded.avif_file.windows(4).any(|bytes| bytes == b"ftyp"));
    }

    #[test]
    fn encode_hdr_still_picture_emits_hdr_signaling() {
        let pixels = [1.0_f32, 0.5, 0.25, 1.0, 4.0, 2.0, 1.0, 1.0];
        let image = RgbaF32Image::new(2, 1, &pixels).unwrap();
        let encoded = Encoder::new()
            .encode_hdr_rgba_f32(&image, &HdrEncodingOptions::default())
            .unwrap();

        let parsed = avif_parse::read_avif(&mut encoded.avif_file.as_slice()).unwrap();
        let metadata = parsed.primary_item_metadata().unwrap();
        assert!(metadata.still_picture);
        assert_eq!(metadata.bit_depth, 10);
        assert_eq!(metadata.chroma_subsampling, (false, false));

        let clli = parsed.content_light_level.expect("clli");
        assert!(clli.max_content_light_level >= 800);
        assert!(clli.max_pic_average_light_level >= 100);

        let mdcv = parsed.mastering_display.expect("mdcv");
        assert_eq!(mdcv.max_luminance, 10_000_000);
        assert_eq!(mdcv.min_luminance, 50);
        assert_eq!(mdcv.primaries[0].0, 8500);
        assert_eq!(mdcv.primaries[0].1, 39850);
        assert_eq!(mdcv.primaries[1].0, 6550);
        assert_eq!(mdcv.primaries[1].1, 2300);
        assert_eq!(mdcv.primaries[2].0, 35400);
        assert_eq!(mdcv.primaries[2].1, 14600);

        assert!(encoded.avif_file.windows(4).any(|bytes| bytes == b"colr"));
        assert!(encoded.avif_file.windows(4).any(|bytes| bytes == b"clli"));
        assert!(encoded.avif_file.windows(4).any(|bytes| bytes == b"mdcv"));
        assert!(
            encoded
                .avif_file
                .windows(4)
                .any(|bytes| bytes == [0, 9, 0, 16])
        );
    }

    #[test]
    fn derived_hdr_content_light_uses_actual_peak_for_maxcll() {
        let pixels = [0.5_f32, 0.5, 0.5, 1.0, 1.0, 1.0, 1.0, 1.0];
        let image = RgbaF32Image::new(2, 1, &pixels).unwrap();
        let encoded = Encoder::new()
            .encode_hdr_rgba_f32(&image, &HdrEncodingOptions::default())
            .unwrap();

        let parsed = avif_parse::read_avif(&mut encoded.avif_file.as_slice()).unwrap();
        let clli = parsed.content_light_level.expect("clli");
        assert_eq!(clli.max_content_light_level, 203);
    }

    #[test]
    fn derived_hdr_content_light_uses_full_frame_average_for_maxfall() {
        let pixels = [
            4.0_f32, 4.0, 4.0, 1.0, 4.0, 4.0, 4.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0,
        ];
        let image = RgbaF32Image::new(2, 2, &pixels).unwrap();
        let encoded = Encoder::new()
            .encode_hdr_rgba_f32(&image, &HdrEncodingOptions::default())
            .unwrap();

        let parsed = avif_parse::read_avif(&mut encoded.avif_file.as_slice()).unwrap();
        let clli = parsed.content_light_level.expect("clli");
        assert_eq!(clli.max_pic_average_light_level, 406);
    }
}
