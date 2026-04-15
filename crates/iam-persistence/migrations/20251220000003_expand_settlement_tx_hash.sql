-- Expand transaction_hash column to accommodate Solana signatures
ALTER TABLE settlements ALTER COLUMN transaction_hash TYPE varchar(128);
