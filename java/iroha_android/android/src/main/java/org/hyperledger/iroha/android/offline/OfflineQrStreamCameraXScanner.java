package org.hyperledger.iroha.android.offline;

import androidx.camera.core.ImageAnalysis;
import androidx.camera.core.ImageProxy;
import com.google.zxing.BarcodeFormat;
import com.google.zxing.BinaryBitmap;
import com.google.zxing.DecodeHintType;
import com.google.zxing.MultiFormatReader;
import com.google.zxing.PlanarYUVLuminanceSource;
import com.google.zxing.Result;
import com.google.zxing.common.HybridBinarizer;
import java.nio.ByteBuffer;
import java.util.Collections;
import java.util.EnumMap;
import java.util.Map;

/** CameraX analyzer that decodes QR stream frames and feeds them into the decoder. */
public final class OfflineQrStreamCameraXScanner implements ImageAnalysis.Analyzer {
  /** Callback interface for decoded frames and payloads. */
  public interface Listener {
    void onProgress(OfflineQrStream.DecodeResult result);

    void onPayload(byte[] payload);

    void onError(Exception error);
  }

  private static final Map<DecodeHintType, Object> HINTS;

  static {
    final Map<DecodeHintType, Object> hints = new EnumMap<>(DecodeHintType.class);
    hints.put(
        DecodeHintType.POSSIBLE_FORMATS, Collections.singletonList(BarcodeFormat.QR_CODE));
    HINTS = Collections.unmodifiableMap(hints);
  }

  private final OfflineQrStream.Decoder decoder;
  private final MultiFormatReader reader;
  private final Listener listener;

  public OfflineQrStreamCameraXScanner(
      final OfflineQrStream.Decoder decoder, final Listener listener) {
    this.decoder = decoder;
    this.listener = listener;
    this.reader = new MultiFormatReader();
    this.reader.setHints(HINTS);
  }

  @Override
  public void analyze(final ImageProxy image) {
    try {
      final byte[] bytes = decodeFrameBytes(image);
      if (bytes == null) {
        return;
      }
      final OfflineQrStream.DecodeResult result = decoder.ingest(bytes);
      listener.onProgress(result);
      if (result.isComplete() && result.payload() != null) {
        listener.onPayload(result.payload());
      }
    } catch (Exception error) {
      listener.onError(error);
    } finally {
      image.close();
    }
  }

  private byte[] decodeFrameBytes(final ImageProxy image) {
    final byte[] luminance = extractLuminance(image);
    if (luminance == null) {
      return null;
    }
    final PlanarYUVLuminanceSource source =
        new PlanarYUVLuminanceSource(
            luminance, image.getWidth(), image.getHeight(), 0, 0, image.getWidth(), image.getHeight(), false);
    final BinaryBitmap bitmap = new BinaryBitmap(new HybridBinarizer(source));
    try {
      final Result result = reader.decodeWithState(bitmap);
      reader.reset();
      final byte[] raw = result.getRawBytes();
      if (raw != null && raw.length > 0) {
        return raw;
      }
      final String text = result.getText();
      if (text == null || text.isBlank()) {
        return null;
      }
      return OfflineQrStream.TextCodec.decode(text, OfflineQrStream.FrameEncoding.BASE64);
    } catch (Exception error) {
      reader.reset();
      return null;
    }
  }

  private static byte[] extractLuminance(final ImageProxy image) {
    if (image.getPlanes().length == 0) {
      return null;
    }
    final ImageProxy.PlaneProxy plane = image.getPlanes()[0];
    final ByteBuffer buffer = plane.getBuffer();
    final byte[] data = new byte[buffer.remaining()];
    buffer.get(data);
    final int width = image.getWidth();
    final int height = image.getHeight();
    final int rowStride = plane.getRowStride();
    final int pixelStride = plane.getPixelStride();
    if (pixelStride == 1 && rowStride == width) {
      return data;
    }
    final byte[] out = new byte[width * height];
    int outputOffset = 0;
    int inputOffset = 0;
    for (int row = 0; row < height; row++) {
      int pixelOffset = inputOffset;
      for (int col = 0; col < width; col++) {
        out[outputOffset++] = data[pixelOffset];
        pixelOffset += pixelStride;
      }
      inputOffset += rowStride;
    }
    return out;
  }
}
