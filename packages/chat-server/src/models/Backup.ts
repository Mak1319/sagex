import { Schema, model } from 'mongoose';

/**
 * Encrypted backup blob metadata. Blob itself is opaque ciphertext;
 * small blobs stored inline, large ones referenced via blobRef (S3/GridFS key).
 */
const backupSchema = new Schema(
  {
    ownerUserId: { type: Schema.Types.ObjectId, ref: 'User', required: true, index: true },
    deviceId: { type: String, required: true },
    blobRef: { type: String, default: null },
    blobInlineCiphertext: { type: String, default: null }, // base64, only for small backups
    blobCiphertextHash: { type: String, required: true }, // sha256 of ciphertext for integrity
    sizeBytes: { type: Number, required: true },
    caKeyIdRef: { type: String, default: null },
  },
  { timestamps: true }
);

backupSchema.index({ ownerUserId: 1, createdAt: -1 });

export const Backup = model('Backup', backupSchema);
