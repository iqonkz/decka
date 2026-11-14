//! Picture helpers on top of Windows.Graphics.Imaging (no image library): decode, scale, encode in memory.
//! Callers must be on a thread where WinRT/COM is initialised.

use windows::core::Interface;
use windows::Foundation::{PropertyType, PropertyValue};
use windows::Graphics::Imaging::{
    BitmapAlphaMode, BitmapDecoder, BitmapEncoder, BitmapInterpolationMode, BitmapPixelFormat, BitmapPropertySet, BitmapTransform, BitmapTypedValue, ColorManagementMode, ExifOrientationMode,
};
use windows::Storage::Streams::{DataReader, DataWriter, IInputStream, InMemoryRandomAccessStream};

pub type Result<T> = windows::core::Result<T>;

pub fn read_all(stream: &impl Interface, size: u32) -> Result<Vec<u8>> {
    let input: IInputStream = stream.cast()?;
    let reader = DataReader::CreateDataReader(&input)?;
    reader.LoadAsync(size)?.join()?;
    let mut bytes = vec![0u8; size as usize];
    reader.ReadBytes(&mut bytes)?;
    reader.DetachStream()?;
    Ok(bytes)
}

pub fn to_stream(bytes: &[u8]) -> Result<InMemoryRandomAccessStream> {
    let stream = InMemoryRandomAccessStream::new()?;
    let writer = DataWriter::CreateDataWriter(&stream)?;
    writer.WriteBytes(bytes)?;
    writer.StoreAsync()?.join()?;
    writer.DetachStream()?;
    stream.Seek(0)?;
    Ok(stream)
}

/// Pixel size of an encoded picture.
pub fn decoded_size(bytes: &[u8]) -> Result<(u32, u32)> {
    let decoder = BitmapDecoder::CreateAsync(&to_stream(bytes)?)?.join()?;
    Ok((decoder.PixelWidth()?, decoder.PixelHeight()?))
}

/// The picture scaled (Fant) to `width` x `height`, as BGRA pixels with the given alpha mode.
pub fn scaled_pixels(bytes: &[u8], width: u32, height: u32, alpha: BitmapAlphaMode) -> Result<Vec<u8>> {
    let decoder = BitmapDecoder::CreateAsync(&to_stream(bytes)?)?.join()?;
    let transform = BitmapTransform::new()?;
    transform.SetScaledWidth(width)?;
    transform.SetScaledHeight(height)?;
    transform.SetInterpolationMode(BitmapInterpolationMode::Fant)?;
    let pixels = decoder
        .GetPixelDataTransformedAsync(BitmapPixelFormat::Bgra8, alpha, &transform, ExifOrientationMode::IgnoreExifOrientation, ColorManagementMode::DoNotColorManage)?
        .join()?
        .DetachPixelData()?;
    Ok(pixels.to_vec())
}

/// Encodes BGRA pixels with the given encoder id (`BitmapEncoder::PngEncoderId()` etc.).
/// `quality` (0..1) only applies to JPEG.
pub fn encode(encoder_id: windows::core::GUID, quality: Option<f32>, alpha: BitmapAlphaMode, pixels: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
    let out = InMemoryRandomAccessStream::new()?;
    let encoder = match quality {
        Some(q) => {
            let options = BitmapPropertySet::new()?;
            let value = BitmapTypedValue::Create(&PropertyValue::CreateSingle(q)?, PropertyType::Single)?;
            options.Insert(&windows::core::HSTRING::from("ImageQuality"), &value)?;
            BitmapEncoder::CreateWithEncodingOptionsAsync(encoder_id, &out, &options)?.join()?
        }
        None => BitmapEncoder::CreateAsync(encoder_id, &out)?.join()?,
    };
    encoder.SetPixelData(BitmapPixelFormat::Bgra8, alpha, width, height, 96.0, 96.0, pixels)?;
    encoder.FlushAsync()?.join()?;
    let size = out.Size()? as u32;
    out.Seek(0)?;
    read_all(&out, size)
}

pub fn encode_png(pixels: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
    encode(BitmapEncoder::PngEncoderId()?, None, BitmapAlphaMode::Premultiplied, pixels, width, height)
}

pub fn encode_jpeg(pixels: &[u8], width: u32, height: u32, quality: f32) -> Result<Vec<u8>> {
    encode(BitmapEncoder::JpegEncoderId()?, Some(quality), BitmapAlphaMode::Ignore, pixels, width, height)
}
