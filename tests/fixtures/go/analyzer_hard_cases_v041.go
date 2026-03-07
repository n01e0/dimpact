package sample

import "context"

type Runner interface {
    Run(ctx context.Context) error
}

type Embedded struct{}

func (Embedded) Trace(msg string) {
    _ = msg
}

type GenericService[T any] struct {
    Embedded // embedded receiver source
    next Runner
}

func Chain[T any](v T, fn func(T) T) T {
    return fn(v)
}

func (s *GenericService[T]) Execute(v T) error {
    s.Trace("exec")
    _ = Chain[T](v, func(x T) T { return x })
    return s.next.Run(context.Background())
}
