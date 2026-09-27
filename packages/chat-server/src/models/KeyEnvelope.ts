import { Schema, model } from 'mongoose';

/**
 * Opaque key-sync envelope issued by the external CA.
 * Server never decrypts `envelopeCiphertext` — treats it as opaque base64.
 */
const keyEnvelopeSchema = new Schema(
  {
    ownerUserId: { type: Schema.Types.ObjectId, ref: 'User', required: true, index: true },
    deviceId: { type: String, required: true, index: true },
    caKeyId: { type: String, required: true, index: true },
    keyType: { type: String, enum: ['identity', 'signed_prekey', 'one_time_prekey', 'group', 'other'], default: 'other' },
    envelopeCiphertext: { type: String, required: true }, // base64, size-capped in controller
    sigRef: { type: String, default: null }, // opaque CA signature reference
    version: { type: Number, default: 1 },
  },
  { timestamps: true }
);

keyEnvelopeSchema.index({ ownerUserId: 1, deviceId: 1, caKeyId: 1 }, { unique: true });

export const KeyEnvelope = model('KeyEnvelope', keyEnvelopeSchema);
