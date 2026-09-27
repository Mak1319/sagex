import { Schema, model } from 'mongoose';

/**
 * Ciphertext-only message. Server stores/relays `ciphertext` verbatim.
 */
const messageSchema = new Schema(
  {
    conversationId: { type: Schema.Types.ObjectId, ref: 'Conversation', required: true, index: true },
    senderId: { type: Schema.Types.ObjectId, ref: 'User', required: true, index: true },
    deviceId: { type: String, required: true },
    ciphertext: { type: String, required: true }, // base64 E2EE payload
    nonceRef: { type: String, default: null },
    caKeyIdRef: { type: String, default: null },
    serverTimestamp: { type: Date, default: () => new Date(), index: true },
  },
  { timestamps: true }
);

messageSchema.index({ conversationId: 1, serverTimestamp: 1 });

export const Message = model('Message', messageSchema);
