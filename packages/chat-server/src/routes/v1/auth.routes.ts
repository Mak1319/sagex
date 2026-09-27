import { Router } from 'express';
import { authLimiter } from '../../middleware/common.js';
import { requireAuth } from '../../middleware/auth.js';
import { signup, login, deviceLogin, refresh, logout, me } from '../../controllers/authController.js';

export const authRoutes = Router();

authRoutes.post('/signup', authLimiter);
authRoutes.post('/signup', signup);
authRoutes.post('/login', authLimiter, login);
authRoutes.post('/device', authLimiter, deviceLogin);
authRoutes.post('/refresh', authLimiter, refresh);
authRoutes.post('/logout', logout);
authRoutes.get('/me', requireAuth, me);
