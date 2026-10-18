-- Migration: create refund approvals and audit trail
-- Supports requiring an approved refund/dispute decision before refund execution
-- and records who requested, approved and executed every refund.

-- Refund approval / decision record.
-- One row per refund request. A refund may only be executed when an approved
-- decision exists (approved = TRUE) and the order is in a refundable state.
CREATE TABLE IF NOT EXISTS refund_approvals (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    order_id UUID NOT NULL REFERENCES orders (id) ON DELETE CASCADE,

    -- Lifecycle status of the refund request.
    -- pending  -> requested, awaiting decision
    -- approved -> decision approved, may be executed
    -- rejected -> decision denied, must not be executed
    -- executed -> refund has been executed
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'approved', 'rejected', 'executed')),

    -- Approved flag for the refund/dispute decision.
    approved BOOLEAN NOT NULL DEFAULT FALSE,

    -- Amount requested for refund (in the order's currency minor units).
    amount BIGINT,

    -- Reason / notes supplied with the request or decision.
    reason TEXT,

    -- Who requested the refund.
    requested_by UUID,
    requested_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    -- Who approved (or rejected) the refund decision.
    approved_by UUID,
    approved_at TIMESTAMPTZ,

    -- Who executed the refund.
    executed_by UUID,
    executed_at TIMESTAMPTZ,

    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    -- An approval must record who approved it and when.
    CONSTRAINT refund_approvals_approved_fields CHECK (
        approved = FALSE
        OR (approved_by IS NOT NULL AND approved_at IS NOT NULL)
    ),

    -- An executed refund must record who executed it and when, and must be approved.
    CONSTRAINT refund_approvals_executed_fields CHECK (
        status <> 'executed'
        OR (
            approved = TRUE
            AND executed_by IS NOT NULL
            AND executed_at IS NOT NULL
        )
    )
);

-- Fast lookup of the latest decision for an order.
CREATE INDEX IF NOT EXISTS idx_refund_approvals_order_id
    ON refund_approvals (order_id);

CREATE INDEX IF NOT EXISTS idx_refund_approvals_order_status
    ON refund_approvals (order_id, status);

-- At most one active (pending/approved) refund request per order at a time,
-- preventing concurrent refund requests racing each other.
CREATE UNIQUE INDEX IF NOT EXISTS idx_refund_approvals_one_active_per_order
    ON refund_approvals (order_id)
    WHERE status IN ('pending', 'approved');

-- Audit columns on orders recording the most recent refund lifecycle actors.
ALTER TABLE orders
    ADD COLUMN IF NOT EXISTS refund_requested_by UUID,
    ADD COLUMN IF NOT EXISTS refund_requested_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS refund_approved_by UUID,
    ADD COLUMN IF NOT EXISTS refund_approved_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS refund_executed_by UUID,
    ADD COLUMN IF NOT EXISTS refund_executed_at TIMESTAMPTZ;

-- Prevent refund and payment release from happening concurrently.
-- A released payment cannot be refunded; a refunded order cannot be released.
ALTER TABLE orders
    ADD CONSTRAINT orders_refund_release_exclusive CHECK (
        NOT (refund_executed_at IS NOT NULL AND released_at IS NOT NULL)
    );