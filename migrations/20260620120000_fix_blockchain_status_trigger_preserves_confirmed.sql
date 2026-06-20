-- Fix: the blockchain-status trigger downgraded an app-set 'confirmed' to 'submitted'.
--
-- update_blockchain_status_on_hash() fires BEFORE UPDATE and, whenever a row's
-- blockchain_tx_signature transitions NULL -> set, forces NEW.blockchain_status
-- = 'submitted'. IAM's mark_user_onboarded() writes the tx signature AND
-- blockchain_status = 'confirmed' in a SINGLE UPDATE, so the trigger clobbered
-- 'confirmed' back to 'submitted' in the same statement (confirmed_at and
-- submitted_at end up identical to the microsecond). Every confirmed on-chain
-- registration was therefore mislabeled 'submitted'.
--
-- The function's own comment already states the intent ("...and status is still
-- pending"), but the guard was missing. Add it so an explicit terminal status
-- (confirmed/failed) set in the same UPDATE is preserved. Shared by the users,
-- settlements, and trading_orders triggers — the pending-only guard is correct
-- for all (only a still-pending row should auto-advance to submitted).

CREATE OR REPLACE FUNCTION update_blockchain_status_on_hash() RETURNS TRIGGER AS $$
BEGIN
    -- Only auto-advance pending -> submitted on first tx-signature set; never
    -- downgrade a status the application explicitly set (e.g. 'confirmed').
    IF NEW.blockchain_tx_signature IS NOT NULL
       AND OLD.blockchain_tx_signature IS NULL
       AND NEW.blockchain_status = 'pending' THEN
        NEW.blockchain_status = 'submitted';
        NEW.blockchain_submitted_at = NOW();
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Backfill rows already mislabeled: tx confirmed on-chain (registered + pda +
-- confirmed_at all set) but status stuck at 'submitted'.
UPDATE users
SET blockchain_status = 'confirmed'
WHERE blockchain_registered = true
  AND user_account_pda IS NOT NULL
  AND blockchain_confirmed_at IS NOT NULL
  AND blockchain_status = 'submitted';
