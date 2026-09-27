import type { NextFunction, Request, Response } from 'express';
import type { ZodSchema } from 'zod';

export function validateBody(schema: ZodSchema) {
  return (req: Request, res: Response, next: NextFunction): void => {
    const parsed = schema.safeParse(req.body);
    if (!parsed.success) {
      res.status(400).json({
        error: { code: 'validation_error', message: 'Invalid request body', details: parsed.error.flatten() },
      });
      return;
    }
    req.body = parsed.data;
    next();
  };
}

export function validateQuery(schema: ZodSchema) {
  return (req: Request, res: Response, next: NextFunction): void => {
    const parsed = schema.safeParse(req.query);
    if (!parsed.success) {
      res.status(400).json({
        error: { code: 'validation_error', message: 'Invalid query params', details: parsed.error.flatten() },
      });
      return;
    }
    (req as unknown as { validatedQuery: unknown }).validatedQuery = parsed.data;
    next();
  };
}
