-- #1: self-registered accounts must verify their email before logging in.
-- Existing accounts (admins, imports, the demo seed) keep the default `true`.
ALTER TABLE "usuarios" ADD COLUMN "correo_verificado" boolean NOT NULL DEFAULT true;