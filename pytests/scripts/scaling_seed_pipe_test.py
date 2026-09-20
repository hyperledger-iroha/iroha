"""Actual anonymous seed pipes with no native child, launch or signal."""
import fcntl
import os
import stat

import pytest
import scaling_seed_pipe as transport
from scaling_generator_fixture import generator_setup, SEED
from scaling_readiness_fixture import command, ready_setup


def test_exact_ready_readonly_nonblocking_seed_and_eof_with_no_retained_text():
    owner=transport.DevelopmentSeedPipe(SEED)
    fd=owner.fd;pin=os.fstat(fd)
    assert stat.S_ISFIFO(pin.st_mode) and pin.st_uid==os.geteuid()
    assert fcntl.fcntl(fd,fcntl.F_GETFL)&os.O_ACCMODE==os.O_RDONLY
    assert not os.get_blocking(fd) and not os.get_inheritable(fd)
    assert SEED not in repr(owner)
    assert os.read(fd,65)==SEED.encode() and os.read(fd,1)==b''
    owner.validate();owner.close();owner.close()
    with pytest.raises(OSError):os.fstat(fd)
    with pytest.raises(transport.SeedPipeError):owner.validate()


@pytest.mark.parametrize('seed',[None,True,64,b'a'*64,'','a'*63,'A'*64,'0'*64,'private-input'])
def test_bad_seed_rejected_before_allocating_any_pipe(monkeypatch,seed):
    calls=[]
    monkeypatch.setattr(transport.os,'pipe',lambda:calls.append('unexpected'))
    with pytest.raises(transport.SeedPipeError) as error:transport.DevelopmentSeedPipe(seed)
    assert str(error.value)=='fixed_scaling_seed_pipe_failed' and calls==[]


@pytest.mark.parametrize('short_write',[False,True])
def test_write_failure_or_partial_write_closes_both_original_ends(monkeypatch,short_write):
    original=os.pipe;captured=[]
    def pipe():
        result=original();captured.extend(result);return result
    def write(fd,raw):
        assert raw==SEED.encode()
        if short_write:return 63
        raise OSError('SECRET_WRITE_ERROR')
    monkeypatch.setattr(transport.os,'pipe',pipe);monkeypatch.setattr(transport.os,'write',write)
    with pytest.raises(transport.SeedPipeError) as error:transport.DevelopmentSeedPipe(SEED)
    assert str(error.value)=='fixed_scaling_seed_pipe_failed'
    for fd in captured:
        with pytest.raises(OSError):os.fstat(fd)


def test_reused_original_fd_fails_validation_and_close_preserves_foreign(tmp_path):
    owner=transport.DevelopmentSeedPipe(SEED);fd=owner.fd
    path=tmp_path/'foreign';path.write_bytes(b'foreign')
    foreign=os.open(path,os.O_RDONLY)
    try:
        os.dup2(foreign,fd,inheritable=False)
        with pytest.raises(transport.SeedPipeError):owner.validate()
        owner.close()
        assert os.read(fd,7)==b'foreign'
    finally:
        os.close(fd);os.close(foreign)


def test_changed_descriptor_flags_fail_without_closing_replacement_slot():
    owner=transport.DevelopmentSeedPipe(SEED);fd=owner.fd
    os.set_blocking(fd,True)
    with pytest.raises(transport.SeedPipeError):owner.validate()
    owner.close();assert os.get_blocking(fd);os.close(fd)


@pytest.mark.parametrize('kind',['wrong_role','wrong_fd_argv','inline_seed','blocking','write_end'])
def test_existing_command_owner_accepts_pipe_only_for_exact_generator_handoff(ready_setup,kind):
    c=ready_setup;reader,writer=os.pipe();os.set_blocking(reader,False)
    fd=writer if kind=='write_end' else reader
    argv=(str(c.images[1].path),'localnet','--seed-fd',str(fd))
    role='other' if kind=='wrong_role' else 'generator'
    if kind=='wrong_fd_argv':argv=(*argv[:-1],str(fd+1))
    if kind=='inline_seed':argv=(*argv,'--seed',SEED)
    if kind=='blocking':os.set_blocking(reader,True)
    owner=command.BoundedCommand(c.images[1],c.commands,c.clock.end(60),lambda:None)
    try:
        with pytest.raises(command.CommandError):owner.run(role,argv,(fd,),100)
        assert not c.commands.calls
    finally:os.close(reader);os.close(writer)


@pytest.mark.parametrize('failure',['spawn','timeout','reuse'])
def test_generator_failure_retains_pipe_until_original_child_reap(generator_setup,failure):
    c=generator_setup;owner=c.create();foreign=None
    if failure=='spawn':c.factory.fail_spawn=0
    if failure=='timeout':c.factory.after_spawn=lambda child:setattr(child,'stall',True)
    if failure=='reuse':
        path=c.root/'foreign-seed-slot';path.write_bytes(b'foreign');foreign=os.open(path,os.O_RDONLY)
        def replace(child):os.dup2(foreign,int(c.factory.calls[-1][0][c.factory.calls[-1][0].index('--seed-fd')+1]),inheritable=False)
        c.factory.after_spawn=replace
    try:
        with pytest.raises(Exception) as error:owner.generate(SEED)
        assert str(error.value)=='fixed_generator_failed'
        pipe=owner._seed_pipe;assert pipe is not None
        fd=pipe._reader
        assert all(SEED not in repr(argv) for argv,_ in c.factory.calls)
        if failure=='timeout':
            assert owner.cleanup(c.clock.end())==('generator',)
            with pytest.raises(Exception):owner.close()
            assert not pipe._closed
            for child in c.factory.children:child.returncode=0
        else:owner.cleanup(c.clock.end())
        owner.close();assert pipe._closed and owner._seed_pipe is None
        if failure=='reuse':assert os.pread(fd,7,0)==b'foreign'
        else:
            with pytest.raises(OSError):os.fstat(fd)
    finally:
        if foreign is not None:
            os.close(fd);os.close(foreign)


def test_seed_owner_cannot_be_readmitted_with_another_secret():
    owner=transport.DevelopmentSeedPipe(SEED);fd=owner.fd
    try:
        with pytest.raises(transport.SeedPipeError):owner.__init__('cd'*32)
        assert owner.fd==fd and os.read(fd,65)==SEED.encode() and os.read(fd,1)==b''
    finally:owner.close()
