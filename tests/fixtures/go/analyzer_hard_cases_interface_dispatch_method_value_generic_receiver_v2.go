package sample

import "context"

type Handler interface {
	Handle(ctx context.Context) error
}

type Pipeline[T any] struct {
	inner Handler
}

func (p *Pipeline[T]) callInner(ctx context.Context) error {
	return p.inner.Handle(ctx)
}

func (p *Pipeline[T]) Execute(v T) error {
	_ = v
	h := p.inner
	if err := h.Handle(context.Background()); err != nil {
		return err
	}
	methodValue := h.Handle
	if err := methodValue(context.Background()); err != nil {
		return err
	}
	runner := p.inner.Handle
	if err := runner(context.Background()); err != nil {
		return err
	}
	bridge := (*Pipeline[T]).callInner
	return bridge(p, context.Background())
}
