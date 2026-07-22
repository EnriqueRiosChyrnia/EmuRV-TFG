# Plugin DUT de EmuRV para RISCOF (RT-3).
#
# Adaptado del template `riscof setup --dutname emurv`:
#   - toolchain: riscv64-elf-gcc (brew; el nombre no depende del xlen) con
#     `_zicsr_zifencei` agregado al march — GCC >= 12 exige las extensiones
#     explícitas y arch_test.h usa CSRs siempre.
#   - ejecución: riscof_runner (src-tauri/src/bin/) — carga por elf_import,
#     corre la Machine hasta EbreakPause y vuelca la signature.

import os
import logging

import riscof.utils as utils
from riscof.pluginTemplate import pluginTemplate

logger = logging.getLogger()


class emurv(pluginTemplate):
    __model__ = "emurv"
    __version__ = "0.1.0"

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        config = kwargs.get('config')
        if config is None:
            logger.error("Config node for emurv missing.")
            raise SystemExit(1)
        self.dut_exe = os.path.abspath(
            os.path.join(config['PATH'] if 'PATH' in config else "", "riscof_runner"))
        self.num_jobs = str(config['jobs'] if 'jobs' in config else 1)
        self.pluginpath = os.path.abspath(config['pluginpath'])
        self.isa_spec = os.path.abspath(config['ispec'])
        self.platform_spec = os.path.abspath(config['pspec'])
        self.target_run = not ('target_run' in config and config['target_run'] == '0')

    def initialise(self, suite, work_dir, archtest_env):
        self.work_dir = work_dir
        self.suite_dir = suite
        self.compile_cmd = 'riscv64-elf-gcc -march={0} \
         -static -mcmodel=medany -fvisibility=hidden -nostdlib -nostartfiles -g\
         -T ' + self.pluginpath + '/env/link.ld\
         -I ' + self.pluginpath + '/env/\
         -I ' + archtest_env + ' {1} -o {2} {3}'

    def build(self, isa_yaml, platform_yaml):
        ispec = utils.load_yaml(isa_yaml)['hart0']
        self.xlen = ('64' if 64 in ispec['supported_xlen'] else '32')
        if self.xlen != '32':
            logger.error("EmuRV DUT: solo RV32 en RT-3.")
            raise SystemExit(1)
        self.compile_cmd = self.compile_cmd + ' -mabi=ilp32 '
        if not os.path.isfile(self.dut_exe):
            logger.error(self.dut_exe + ": no existe. Correr scripts/run-riscof.sh setup.")
            raise SystemExit(1)

    def runTests(self, testList):
        makefile = os.path.join(self.work_dir, "Makefile." + self.name[:-1])
        if os.path.exists(makefile):
            os.remove(makefile)
        make = utils.makeUtil(makefilePath=makefile)
        make.makeCommand = 'make -k -j' + self.num_jobs

        for testname in testList:
            testentry = testList[testname]
            test = testentry['test_path']
            test_dir = testentry['work_dir']
            elf = 'dut.elf'
            sig_file = os.path.join(test_dir, self.name[:-1] + ".signature")

            # GCC >= 12: Zicsr/Zifencei explícitas (arch_test.h usa CSRs y
            # los tests de I incluyen fence.i).
            march = testentry['isa'].lower()
            for ext in ('_zicsr', '_zifencei'):
                if ext not in march:
                    march += ext
            compile_macros = ' -D' + " -D".join(testentry['macros'])
            cmd = self.compile_cmd.format(march, test, elf, compile_macros)

            if self.target_run:
                # Si el runner falla (test incompatible por diseño, p.ej.
                # cebreak-01: EmuRV define ebreak como pausa del depurador,
                # RP-1), se deja una signature-marcador para que RISCOF
                # reporte Failed en vez de abortar la comparación de toda
                # la suite. El motivo queda en el .log del test.
                simcmd = ('{0} {1} --signature {2} 2> runner.log || '
                          'echo "-- riscof_runner fallo: ver runner.log --" > {2}'
                          .format(self.dut_exe, elf, sig_file))
            else:
                simcmd = 'echo "NO RUN"'

            execute = '@cd {0}; {1}; {2};'.format(test_dir, cmd, simcmd)
            make.add_target(execute)

        make.execute_all(self.work_dir)
        if not self.target_run:
            raise SystemExit(0)
