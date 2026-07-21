'use strict';

const crypto = require('crypto');
const config = require('./config');

const COOKIE_NAME = 'mg_session';

function sign(value) {
  return crypto.createHmac('sha256', config.SESSION_SECRET).update(value).digest('hex');
}

function safeEqual(a, b) {
  const ba = Buffer.from(String(a));
  const bb = Buffer.from(String(b));
  if (ba.length !== bb.length) return false;
  return crypto.timingSafeEqual(ba, bb);
}

function createSessionCookie(res) {
  const expires = Date.now() + config.SESSION_DAYS * 24 * 60 * 60 * 1000;
  const value = `${expires}.${sign(String(expires))}`;
  res.cookie(COOKIE_NAME, value, {
    httpOnly: true,
    sameSite: 'lax',
    maxAge: config.SESSION_DAYS * 24 * 60 * 60 * 1000,
    path: '/',
  });
}

function clearSessionCookie(res) {
  res.clearCookie(COOKIE_NAME, { path: '/' });
}

function hasValidSession(req) {
  const raw = req.headers.cookie;
  if (!raw) return false;
  const match = raw
    .split(';')
    .map((c) => c.trim())
    .find((c) => c.startsWith(COOKIE_NAME + '='));
  if (!match) return false;
  const value = decodeURIComponent(match.slice(COOKIE_NAME.length + 1));
  const dot = value.indexOf('.');
  if (dot === -1) return false;
  const expires = value.slice(0, dot);
  const sig = value.slice(dot + 1);
  if (!safeEqual(sig, sign(expires))) return false;
  return Number(expires) > Date.now();
}

function hasValidToken(req) {
  if (!config.API_TOKEN) return false;
  const header = req.headers.authorization || '';
  const bearer = header.startsWith('Bearer ') ? header.slice(7).trim() : '';
  const token = bearer || req.headers['x-api-token'] || '';
  return token ? safeEqual(token, config.API_TOKEN) : false;
}

function requireAuth(req, res, next) {
  if (hasValidToken(req) || hasValidSession(req)) return next();
  res.status(401).json({ error: 'No autorizado' });
}

// Límite de intentos de login: 10 fallos por IP cada 15 minutos.
const failures = new Map();
const WINDOW_MS = 15 * 60 * 1000;
const MAX_FAILURES = 10;

function loginAllowed(ip) {
  const entry = failures.get(ip);
  if (!entry) return true;
  if (Date.now() - entry.first > WINDOW_MS) {
    failures.delete(ip);
    return true;
  }
  return entry.count < MAX_FAILURES;
}

function registerFailure(ip) {
  const entry = failures.get(ip);
  if (!entry || Date.now() - entry.first > WINDOW_MS) {
    failures.set(ip, { first: Date.now(), count: 1 });
  } else {
    entry.count += 1;
  }
}

function checkPassword(password) {
  return Boolean(config.PASSWORD) && safeEqual(password, config.PASSWORD);
}

module.exports = {
  requireAuth,
  createSessionCookie,
  clearSessionCookie,
  hasValidSession,
  loginAllowed,
  registerFailure,
  checkPassword,
};
