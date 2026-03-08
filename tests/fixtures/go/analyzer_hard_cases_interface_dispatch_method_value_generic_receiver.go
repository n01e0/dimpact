package sample

import "context"

type Worker interface {
    Handle(ctx context.Context) error
}

type Box[T any] struct {
    inner Worker
}

func (b *Box[T]) Run(v T) error {
    _ = v
    methodValue := b.inner.Handle
    if err := methodValue(context.Background()); err != nil {
        return err
    }
    return b.inner.Handle(context.Background())
}
