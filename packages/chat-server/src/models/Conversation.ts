import { Schema, model } from 'mongoose';

const conversationSchema = new Schema(
  {
    type: { type: String, enum: ['dm', 'group'], required: true },
    participantIds: { type: [Schema.Types.ObjectId], ref: 'User', required: true },
    createdBy: { type: Schema.Types.ObjectId, ref: 'User', required: true },
    titleCiphertext: { type: String, default: null }, // E2EE: encrypted group title, opaque
    groupKeyEnvelopeCiphertext: { type: String, default: null }, // E2EE: encrypted group key, opaque
    caKeyIdRef: { type: String, default: null },
  },
  { timestamps: true }
);

conversationSchema.index({ participantIds: 1 });

export const Conversation = model('Conversation', conversationSchema);
