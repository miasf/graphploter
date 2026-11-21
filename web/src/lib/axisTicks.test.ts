import { axisTicks, axisTickText } from './axisTicks.ts'

function check(actual: unknown, expected: unknown) {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    throw new Error(`${JSON.stringify(actual)} → expected ${JSON.stringify(expected)}`)
  }
}

const symmetric = axisTicks(-12, 12)
check(symmetric.step, 2)
check(symmetric.grid, [-10, -8, -6, -4, -2, 0, 2, 4, 6, 8, 10])
check(symmetric.labels.filter((tick) => tick.edge).map((tick) => tick.value), [-12, 12])

const uneven = axisTicks(-4.2, 12.3)
check(uneven.labels.filter((tick) => tick.edge).map((tick) => tick.value), [-4.2, 12.3])
check(uneven.grid.every((value) => value > -4.2 && value < 12.3), true)

const small = axisTicks(-2e-12, 3e-12)
check(small.grid.length > 2 && small.grid.every((value) => value > -2e-12 && value < 3e-12), true)
check(axisTickText(0.30000000000000004, 0.1), '0.3')
check(axisTickText(100000000001, 1), '100000000001')
check(axisTickText(100000000002, 1), '100000000002')

console.log('axisTicks.ts: すべて成功')
