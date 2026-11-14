import { CapacitorBarcodeScanner, CapacitorBarcodeScannerTypeHint } from "@capacitor/barcode-scanner";

/** Opens the camera scanner for a QR code. Returns the text, or null when cancelled or on any error. */
export async function scanQr(): Promise<string | null> {
  try {
    const result = await CapacitorBarcodeScanner.scanBarcode({ hint: CapacitorBarcodeScannerTypeHint.QR_CODE });
    return result.ScanResult ? result.ScanResult : null;
  } catch {
    return null;
  }
}
