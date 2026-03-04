package org.hyperledger.iroha.android.offline;

import androidx.camera.core.ImageAnalysis;
import androidx.camera.core.ImageProxy;
import java.nio.ByteBuffer;

/** CameraX analyzer that decodes petal stream frames into QR stream payloads. */
public final class OfflinePetalStreamCameraXScanner implements ImageAnalysis.Analyzer {
  /** Callback interface for decoded frames and payloads. */
  public interface Listener {
    void onProgress(OfflineQrStream.DecodeResult result);

    void onPayload(byte[] payload);

    void onError(Exception error);
  }

  private final OfflineQrStream.Decoder decoder;
  private final OfflinePetalStream.Options options;
  private final Listener listener;
  private Integer resolvedGridSize;

  public OfflinePetalStreamCameraXScanner(
      final OfflineQrStream.Decoder decoder,
      final OfflinePetalStream.Options options,
      final Listener listener) {
    this.decoder = decoder;
    this.options = options == null ? new OfflinePetalStream.Options() : options;
    this.listener = listener;
    this.resolvedGridSize = this.options.gridSize() == 0 ? null : this.options.gridSize();
  }

  public OfflinePetalStreamCameraXScanner(
      final OfflineQrStream.Decoder decoder, final Listener listener) {
    this(decoder, new OfflinePetalStream.Options(), listener);
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
    final int width = image.getWidth();
    final int height = image.getHeight();
    if (resolvedGridSize != null) {
      final OfflinePetalStream.SampleGrid samples =
          OfflinePetalStream.Sampler.sampleGridFromLuma(
              luminance, width, height, resolvedGridSize);
      return OfflinePetalStream.Decoder.decodeSamples(samples, options);
    }
    final OfflinePetalStream.Options autoOptions =
        new OfflinePetalStream.Options(0, options.border(), options.anchorSize());
    for (int candidate : OfflinePetalStream.gridSizes()) {
      try {
        final OfflinePetalStream.SampleGrid samples =
            OfflinePetalStream.Sampler.sampleGridFromLuma(
                luminance, width, height, candidate);
        final byte[] payload = OfflinePetalStream.Decoder.decodeSamples(samples, autoOptions);
        resolvedGridSize = candidate;
        return payload;
      } catch (Exception ignore) {
        // Try next candidate.
      }
    }
    return null;
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
