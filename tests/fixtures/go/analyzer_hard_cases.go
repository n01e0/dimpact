package sample

import (
    "context"
    "fmt"
)

type Logger struct{}

func (Logger) Log(msg string) {
    fmt.Println(msg)
}

type Client struct{}

func (c *Client) Do(ctx context.Context) error {
    _ = ctx
    return nil
}

type Repo[T any] struct {
    client *Client
}

func NewRepo[T any]() *Repo[T] {
    return &Repo[T]{client: &Client{}}
}

type Service[T any] struct {
    Logger // embedded field
    repo *Repo[T]
}

func Map[T any](xs []T, fn func(T) T) []T {
    out := make([]T, 0, len(xs))
    for _, x := range xs {
        out = append(out, fn(x))
    }
    return out
}

func (s *Service[T]) Handle(v T) error {
    s.Log("start")
    _ = s.repo.client.Do(context.Background())
    _ = Map[T]([]T{v}, func(x T) T { return x })
    return nil
}
