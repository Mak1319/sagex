import { Router } from 'express';
import { requireAuth } from '../../middleware/auth.js';
import { upsertKey, listKeys, deleteOneTimeKey } from '../../controllers/keysController.js';
import { createConversation, listConversations, getConversation } from '../../controllers/conversationsController.js';
import { sendMessage, listMessages } from '../../controllers/messagesController.js';
import { putBackup, listBackups, getBackup } from '../../controllers/backupsController.js';

export const keysRoutes = Router();
keysRoutes.use(requireAuth);
keysRoutes.put('/:deviceId', upsertKey);
keysRoutes.get('/:userId', listKeys);
keysRoutes.delete('/:deviceId/one-time/:keyId', deleteOneTimeKey);

export const conversationRoutes = Router();
conversationRoutes.use(requireAuth);
conversationRoutes.post('/', createConversation);
conversationRoutes.get('/', listConversations);
conversationRoutes.get('/:id', getConversation);
conversationRoutes.post('/:id/messages', sendMessage);
conversationRoutes.get('/:id/messages', listMessages);

export const backupRoutes = Router();
backupRoutes.use(requireAuth);
backupRoutes.put('/', putBackup);
backupRoutes.get('/', listBackups);
backupRoutes.get('/:id', getBackup);
